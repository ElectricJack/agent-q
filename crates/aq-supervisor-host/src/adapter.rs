//! Shared Codex mapper; the guardian-owned wrapper retains the provider transport.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use aq_codex_mapper::{LoopbackResponsesEndpoint, Paths, Recipe, Settings};
use aq_session_host::{
    AdapterLaunch, HarnessReadiness, HostAdapter, HostObservation, HostPlan, LocalCommand,
};
use aq_session_protocol::{Digest, HarnessSettings, ModelSettings};
use std::{
    io::Write,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    sync::Arc,
};

pub fn native_recipe(
    harness: &HarnessSettings,
    model: &ModelSettings,
    input: bool,
    paths: Paths<'_>,
    port: u16,
) -> Result<Recipe> {
    let mut recipe = aq_codex_mapper::map(
        Settings {
            harness,
            model,
            terminal_input_enabled: input,
        },
        paths,
    )?;
    let stdin = recipe.arguments.pop().context("missing stdin recipe")?;
    for value in aq_codex_mapper::responses_overrides(LoopbackResponsesEndpoint::new(port)?)
        .into_iter()
        .chain(["cli_auth_credentials_store=\"file\"".into()])
    {
        recipe.arguments.extend(["--config".into(), value]);
    }
    recipe.arguments.push(stdin);
    Ok(recipe)
}
pub struct NativeAdapter {
    pub config: PathBuf,
    pub digest: Digest,
    pub wrapper: PathBuf,
}
impl HostAdapter for NativeAdapter {
    fn prepare(&self, plan: &HostPlan, workspace: &Path) -> Result<AdapterLaunch> {
        let (c, digest) = Config::load(&self.config)?;
        ensure!(
            digest == self.digest && c.host.digest()? == plan.digest()? && workspace == c.workspace,
            "installed adapter admission changed"
        );
        aq_codex_mapper::validate_settings(Settings {
            harness: &plan.harness,
            model: &c.model,
            terminal_input_enabled: plan.terminal.input_enabled,
        })?;
        Ok(AdapterLaunch {
            manifest_digest: plan.digest()?,
            adapter: plan.harness.adapter.clone(),
            cli_version: plan.harness.cli_version.as_str().into(),
            environment: plan.harness.environment.clone(),
            capabilities: plan.harness.required_capabilities.clone(),
            resource_qualification: None,
            command: LocalCommand {
                executable: self.wrapper.clone(),
                arguments: vec![
                    "native".into(),
                    self.config.to_str().context("config path UTF8")?.into(),
                    digest.as_str().into(),
                ],
                environment: Default::default(),
            },
        })
    }
    fn readiness(&self, _: &HostObservation) -> Result<HarnessReadiness> {
        Ok(HarnessReadiness::Unknown)
    }
}
/// Root transport and unprivileged native child share the guardian's kill domain.
/// The image must use a private PID namespace and never mount the Docker socket.
pub async fn native(path: &Path, expected: &str) -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "native host requires root credential separation"
    );
    ensure!(
        unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0,
        "transport dump protection"
    );
    let (c, digest) = Config::load(path)?;
    ensure!(digest.as_str() == expected, "native admission changed");
    let directory =
        PathBuf::from(std::env::var_os("AQ_SESSION_DIRECTORY").context("host directory missing")?);
    ensure!(
        directory.parent() == Some(c.root.as_path()),
        "foreign session directory"
    );
    let startup = crate::config::protected_read(&directory.join("context-0.bin"), 1048576)?;
    c.host.verify_startup(&startup)?;
    let boot = crate::clock()?;
    ensure!(
        boot.machine_boot_id == c.host.machine_boot_id && boot.boot_ms < c.host.deadline_boot_ms,
        "native deadline or boot changed"
    );
    let workspace = std::fs::symlink_metadata(&c.workspace)?;
    ensure!(
        workspace.is_dir()
            && workspace.uid() == 10001
            && c.workspace.canonicalize()? == c.workspace,
        "native workspace ownership"
    );
    let homes = std::fs::symlink_metadata(&c.homes)?;
    ensure!(
        homes.is_dir()
            && homes.uid() == 0
            && homes.mode() & 0o022 == 0
            && c.homes.canonicalize()? == c.homes,
        "native home parent ownership"
    );
    let home = c.homes.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir(&home)?;
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))?;
    let key = Arc::new(crate::credentials::SecretFile::open(c.secret_file.clone())?);
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let origin = format!("http://127.0.0.1:{port}");
    let policy = aq_responses_transport::ResponsesPolicy::new(
        c.host.digest()?,
        &c.model,
        c.host
            .deadline_boot_ms
            .checked_mul(1_000_000)
            .context("deadline overflow")?,
    )?;
    let transport = aq_responses_transport::ScopedResponsesTransport::start_on_listener(
        listener,
        &origin,
        policy,
        aq_responses_transport::UpstreamEndpoint::openai(),
        key,
    )
    .await?;
    let capability = transport.local_capability();
    let auth = zeroize::Zeroizing::new(serde_json::to_vec(
        &serde_json::json!({"OPENAI_API_KEY":std::str::from_utf8(&capability)?}),
    )?);
    let auth_path = home.join("auth.json");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&auth_path)?
        .write_all(&auth)?;
    std::os::unix::fs::chown(&auth_path, Some(10001), Some(10001))?;
    std::os::unix::fs::chown(&home, Some(10001), Some(10001))?;
    // Retain the verified executable descriptor across exec; path replacement
    // cannot substitute a different binary between verification and launch.
    let executable = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&c.native)?;
    let meta = executable.metadata()?;
    ensure!(
        meta.is_file()
            && meta.uid() == 0
            && meta.mode() & 0o022 == 0
            && meta.mode() & 0o111 != 0
            && meta.len() <= 512 * 1024 * 1024,
        "native executable rejected"
    );
    use std::io::Read;
    use std::os::fd::AsRawFd;
    let mut bytes = Vec::new();
    (&executable)
        .take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 512 * 1024 * 1024 && Digest::of(&bytes) == c.native_digest,
        "native digest mismatch"
    );
    drop(bytes);
    let fd = executable.as_raw_fd();
    let exec_path = PathBuf::from(format!("/proc/self/fd/{fd}"));
    let recipe = native_recipe(
        &c.host.harness,
        &c.model,
        c.host.terminal.input_enabled,
        Paths {
            executable: &exec_path,
            workspace: &c.workspace,
            private_home: &home,
        },
        port,
    )?;
    let mut command = std::process::Command::new(recipe.executable);
    command
        .args(recipe.arguments)
        .env_clear()
        .envs(recipe.environment)
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .current_dir(&c.workspace)
        .stdin(std::process::Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0
                || libc::setgroups(0, std::ptr::null()) != 0
                || libc::setgid(10001) != 0
                || libc::setuid(10001) != 0
                || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                || libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    child
        .stdin
        .take()
        .context("native stdin")?
        .write_all(&startup)?;
    let result = tokio::task::spawn_blocking(move || child.wait()).await?;
    transport.stop().await?;
    ensure!(result?.success(), "native process failed");
    Ok(())
}
