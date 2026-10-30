//! The boundary between the update engine and the host: Docker, Compose and
//! the database container. `Runtime` is the trait the engine drives;
//! `DockerCli` implements it with the `docker` CLI.

use aq_release::ReadinessReport;
use async_trait::async_trait;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

/// Which release to fetch: an exact version, or the configured channel's
/// current release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseSelector {
    Version(semver::Version),
    Channel,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
    /// The operation failed; the engine decides what that means for the
    /// update (refuse, abort or roll back).
    #[error("{0}")]
    Failed(String),
    /// The updater is going away mid-operation (shutdown, or a simulated
    /// crash in tests). The engine stops at once and leaves the journal as
    /// written, so `Engine::recover` resumes from the persisted step.
    #[error("interrupted: {0}")]
    Interrupted(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[async_trait]
pub trait Runtime: Send + Sync {
    /// The release's `(manifest bytes, signature bundle bytes)`, unverified.
    async fn fetch_release(&self, selector: ReleaseSelector) -> RuntimeResult<(Vec<u8>, Vec<u8>)>;
    async fn pull(&self, image: &str) -> RuntimeResult<()>;
    async fn image_labels(&self, image: &str) -> RuntimeResult<HashMap<String, String>>;
    async fn free_bytes(&self, path: &Path) -> RuntimeResult<u64>;
    /// Write a consistent database backup to `dest`; returns its size.
    async fn backup(&self, dest: &Path) -> RuntimeResult<u64>;
    /// Replace the database with `src`. Only called with the server stopped.
    async fn restore(&self, src: &Path) -> RuntimeResult<()>;
    /// Start (or replace) the server container with `image`.
    async fn start_server(&self, image: &str) -> RuntimeResult<()>;
    async fn stop_server(&self) -> RuntimeResult<()>;
    async fn wait_healthy(&self, deadline: Duration) -> RuntimeResult<()>;
    async fn readiness(&self) -> RuntimeResult<ReadinessReport>;
    /// The state format recorded in the database's `meta` table; `None` when
    /// there is no database or table yet.
    async fn server_state_format(&self) -> RuntimeResult<Option<u32>>;
}

// ------------------------------------------------------------------ DockerCli

/// `docker pull` and the release fetch.
pub const PULL_TIMEOUT: Duration = Duration::from_secs(600);
/// `pg_dump` and `pg_restore`.
pub const BACKUP_TIMEOUT: Duration = Duration::from_secs(1800);
/// Every other compose, inspect and exec call.
pub const CMD_TIMEOUT: Duration = Duration::from_secs(120);
/// Health status poll interval in `wait_healthy`.
pub const HEALTH_POLL: Duration = Duration::from_secs(2);
/// Largest `manifest.json` or bundle accepted from a release image.
pub const MAX_RELEASE_FILE: usize = 1024 * 1024;
/// Largest stdout captured from any other command.
const MAX_CAPTURE: u64 = 4 * 1024 * 1024;
/// Stderr bytes kept for error messages (`stderr_tail` trims to 2 KiB).
const STDERR_KEEP: usize = 4096;

pub const RELEASE_ENV: &str = "release.env";
const SERVICE_SERVER: &str = "server";
const SERVICE_POSTGRES: &str = "postgres";
const STATE_FORMAT_SQL: &str = "SELECT value FROM meta WHERE key = 'state_format_version'";
// `IF EXISTS`: a restore cut short after the drop can be retried.
const DROP_SQL: &str = "DROP DATABASE IF EXISTS agent_q WITH (FORCE)";
const CREATE_SQL: &str = "CREATE DATABASE agent_q OWNER agent_q";

fn tag_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$").expect("static"))
}

fn container_id_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[0-9a-f]{12,64}$").expect("static"))
}

fn failed(m: impl Into<String>) -> RuntimeError {
    RuntimeError::Failed(m.into())
}

fn digest_ref(image: &str) -> RuntimeResult<&str> {
    if aq_release::is_digest_ref(image) {
        Ok(image)
    } else {
        Err(failed(format!(
            "refusing image reference {:.200?}: not digest-pinned (repo@sha256:<64 hex>)",
            image
        )))
    }
}

fn container_id(id: &str) -> RuntimeResult<&str> {
    if container_id_re().is_match(id) {
        Ok(id)
    } else {
        Err(failed(format!(
            "unexpected container id {id:.100?} from docker"
        )))
    }
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn fsync_parent(path: &Path) -> std::io::Result<()> {
    match path.parent() {
        Some(dir) => std::fs::File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}

/// Write `AQ_SERVER_IMAGE=<image>` to `path` atomically (temp file, fsync,
/// rename, fsync dir), mode 0640. The image must be digest-pinned, which
/// also rules out anything that could add another variable.
pub fn write_release_env(path: &Path, image: &str) -> RuntimeResult<()> {
    let image = digest_ref(image)?;
    write_env_file(path, format!("AQ_SERVER_IMAGE={image}\n").as_bytes())
}

fn write_env_file(path: &Path, body: &[u8]) -> RuntimeResult<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let err = |e: std::io::Error| failed(format!("cannot write {}: {e}", path.display()));
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o640)
        .open(&tmp)
        .map_err(err)?;
    f.set_permissions(std::fs::Permissions::from_mode(0o640))
        .map_err(err)?;
    f.write_all(body).map_err(err)?;
    f.sync_all().map_err(err)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(err)?;
    fsync_parent(path).map_err(err)
}

/// Make sure `release.env` exists so Compose can read it: written with
/// `image` (the active release) when given, otherwise empty. An existing
/// file is never replaced.
pub fn ensure_release_env(path: &Path, image: Option<&str>) -> RuntimeResult<()> {
    if path.exists() {
        return Ok(());
    }
    match image {
        Some(i) => write_release_env(path, i),
        None => write_env_file(path, b""),
    }
}

/// The content of the regular file `name` from a `docker cp <id>:/path -`
/// tar stream, at most `MAX_RELEASE_FILE` bytes.
pub fn extract_tar_file(tar_bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut archive = tar::Archive::new(tar_bytes);
    let entries = archive
        .entries()
        .map_err(|e| format!("not a tar stream: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("bad tar stream: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("bad tar entry name: {e}"))?;
        if path.file_name().and_then(|n| n.to_str()) != Some(name) {
            continue;
        }
        if entry.header().entry_type() != tar::EntryType::Regular {
            return Err(format!("{name} in the release image is not a regular file"));
        }
        if entry.size() > MAX_RELEASE_FILE as u64 {
            return Err(format!(
                "{name} in the release image is larger than {MAX_RELEASE_FILE} bytes"
            ));
        }
        let mut data = Vec::new();
        entry
            .take(MAX_RELEASE_FILE as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| format!("cannot read {name} from the tar stream: {e}"))?;
        if data.len() > MAX_RELEASE_FILE {
            return Err(format!(
                "{name} in the release image is larger than {MAX_RELEASE_FILE} bytes"
            ));
        }
        return Ok(data);
    }
    Err(format!("{name} not found in the release image"))
}

/// Interpret `psql -tAc <state format query>`: a missing database or `meta`
/// table means no state format yet.
pub fn parse_state_format(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> RuntimeResult<Option<u32>> {
    if !success {
        let err = String::from_utf8_lossy(stderr);
        let missing = err.contains("database \"agent_q\" does not exist")
            || err.contains("relation \"meta\" does not exist");
        return if missing {
            Ok(None)
        } else {
            Err(failed(format!(
                "psql (state format) failed: {}",
                stderr_tail(stderr)
            )))
        };
    }
    let text = String::from_utf8_lossy(stdout);
    let value = text.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value.parse().map(Some).map_err(|_| {
        failed(format!(
            "unexpected state_format_version {value:.64?} in the database"
        ))
    })
}

/// The real runtime: the `docker` CLI against the configured Compose project.
/// Every call is a fixed argument vector (no shell), killed when it times
/// out. Image references reach `docker` only as validated digest refs
/// (`pull`, `image inspect`) or through `release.env`; release refs are
/// built from the configured repo and a validated version or channel tag.
#[derive(Debug, Clone)]
pub struct DockerCli {
    docker: std::path::PathBuf,
    compose: crate::config::ComposeConfig,
    release_env: std::path::PathBuf,
    release_repo: String,
    channel: String,
}

struct Ran {
    success: bool,
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Ran {
    fn ok(self, name: &str) -> RuntimeResult<Vec<u8>> {
        if self.success {
            Ok(self.stdout)
        } else {
            Err(failed(format!(
                "{name} failed ({}): {}",
                self.status,
                stderr_tail(&self.stderr)
            )))
        }
    }
}

impl DockerCli {
    pub fn new(
        compose: crate::config::ComposeConfig,
        state_dir: &Path,
        release_repo: String,
        channel: String,
    ) -> Self {
        DockerCli {
            docker: "docker".into(),
            compose,
            release_env: state_dir.join(RELEASE_ENV),
            release_repo,
            channel,
        }
    }

    pub fn from_config(cfg: &crate::config::UpdaterConfig) -> Self {
        Self::new(
            cfg.compose.clone(),
            &cfg.state_dir,
            cfg.release_repo.clone(),
            cfg.channel.clone(),
        )
    }

    /// Use another `docker` binary (tests).
    pub fn with_docker_binary(mut self, docker: std::path::PathBuf) -> Self {
        self.docker = docker;
        self
    }

    pub fn release_env_path(&self) -> &Path {
        &self.release_env
    }

    // ------------------------------------------------- argv (pure, tested)

    /// `compose --project-name <n> --project-directory <d> -f <f>…
    /// --env-file <e>… --env-file <state_dir>/release.env <args>`.
    pub fn compose_argv(&self, args: &[&str]) -> Vec<OsString> {
        let c = &self.compose;
        let mut v: Vec<OsString> = os(&["compose", "--project-name", &c.project_name]);
        v.push("--project-directory".into());
        v.push(c.project_dir.clone().into());
        for f in &c.files {
            v.push("-f".into());
            v.push(f.clone().into());
        }
        for e in c.env_files.iter().chain(std::iter::once(&self.release_env)) {
            v.push("--env-file".into());
            v.push(e.clone().into());
        }
        v.extend(args.iter().map(OsString::from));
        v
    }

    pub fn start_server_argv(&self) -> Vec<OsString> {
        self.compose_argv(&["up", "-d", "--no-deps", "--force-recreate", SERVICE_SERVER])
    }
    pub fn stop_server_argv(&self) -> Vec<OsString> {
        self.compose_argv(&["stop", SERVICE_SERVER])
    }
    pub fn server_container_argv(&self) -> Vec<OsString> {
        self.compose_argv(&["ps", "-q", SERVICE_SERVER])
    }
    pub fn readiness_argv(&self) -> Vec<OsString> {
        self.compose_argv(&["exec", "-T", SERVICE_SERVER, "aq-server", "readiness"])
    }
    fn psql_argv(&self, db: &str, flag: &str, sql: &str) -> Vec<OsString> {
        self.compose_argv(&[
            "exec",
            "-T",
            SERVICE_POSTGRES,
            "psql",
            "-U",
            "agent_q",
            "-d",
            db,
            flag,
            sql,
        ])
    }
    pub fn backup_argv(&self) -> Vec<OsString> {
        self.compose_argv(&[
            "exec",
            "-T",
            SERVICE_POSTGRES,
            "pg_dump",
            "-U",
            "agent_q",
            "-Fc",
            "agent_q",
        ])
    }
    /// Drop, create, then `pg_restore` (fed the dump on stdin).
    pub fn restore_argvs(&self) -> [Vec<OsString>; 3] {
        [
            self.psql_argv("postgres", "-c", DROP_SQL),
            self.psql_argv("postgres", "-c", CREATE_SQL),
            self.compose_argv(&[
                "exec",
                "-T",
                SERVICE_POSTGRES,
                "pg_restore",
                "-U",
                "agent_q",
                "-d",
                "agent_q",
            ]),
        ]
    }
    pub fn state_format_argv(&self) -> Vec<OsString> {
        self.psql_argv("agent_q", "-tAc", STATE_FORMAT_SQL)
    }

    pub fn pull_argv(&self, image: &str) -> RuntimeResult<Vec<OsString>> {
        Ok(os(&["pull", "--quiet", digest_ref(image)?]))
    }
    pub fn labels_argv(&self, image: &str) -> RuntimeResult<Vec<OsString>> {
        Ok(os(&[
            "image",
            "inspect",
            "--format",
            "{{json .Config.Labels}}",
            digest_ref(image)?,
        ]))
    }
    pub fn health_argv(&self, id: &str) -> RuntimeResult<Vec<OsString>> {
        Ok(os(&[
            "inspect",
            "--format",
            "{{.State.Health.Status}}",
            container_id(id)?,
        ]))
    }

    /// `<release_repo>:<version|channel>`, the tag validated as a Docker tag.
    pub fn release_ref(&self, selector: &ReleaseSelector) -> RuntimeResult<String> {
        let tag = match selector {
            ReleaseSelector::Version(v) => v.to_string(),
            ReleaseSelector::Channel => self.channel.clone(),
        };
        if !tag_re().is_match(&tag) {
            return Err(failed(format!("{tag:.140?} is not a valid release tag")));
        }
        Ok(format!("{}:{tag}", self.release_repo))
    }
    /// Only for refs from `release_ref`.
    pub fn release_pull_argv(&self, release_ref: &str) -> Vec<OsString> {
        os(&["pull", "--quiet", release_ref])
    }
    /// Only for refs from `release_ref`. `none` is the (never run) command:
    /// the release image is `FROM scratch`.
    pub fn release_create_argv(&self, release_ref: &str) -> Vec<OsString> {
        os(&["create", release_ref, "none"])
    }
    pub fn cp_argv(&self, id: &str, path: &str) -> RuntimeResult<Vec<OsString>> {
        Ok(os(&["cp", &format!("{}:{path}", container_id(id)?), "-"]))
    }
    pub fn rm_argv(&self, id: &str) -> RuntimeResult<Vec<OsString>> {
        Ok(os(&["rm", container_id(id)?]))
    }

    // --------------------------------------------------------- execution

    /// Run `docker <argv>`; stdout goes to `sink` (at most `cap` bytes when
    /// given). Killed on timeout or when dropped.
    async fn exec_into<W>(
        &self,
        name: &str,
        argv: Vec<OsString>,
        timeout: Duration,
        stdin: std::process::Stdio,
        sink: &mut W,
        cap: Option<u64>,
    ) -> RuntimeResult<(std::process::ExitStatus, Vec<u8>, u64)>
    where
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        use tokio::io::AsyncReadExt;
        let mut child = tokio::process::Command::new(&self.docker)
            .args(&argv)
            .stdin(stdin)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            // Own process group: a Ctrl-C at the terminal reaches only
            // aq-updater, which finishes the step instead of losing it.
            .process_group(0)
            .spawn()
            .map_err(|e| failed(format!("{name}: cannot run {}: {e}", self.docker.display())))?;
        let stdout = child.stdout.take().expect("piped");
        let mut stderr = child.stderr.take().expect("piped");
        let err_task = tokio::spawn(async move {
            let mut keep = Vec::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                keep.extend_from_slice(&buf[..n]);
                if keep.len() > STDERR_KEEP {
                    keep.drain(..keep.len() - STDERR_KEEP);
                }
            }
            keep
        });
        let run = async {
            let copied = match cap {
                Some(c) => tokio::io::copy(&mut stdout.take(c + 1), sink).await,
                None => {
                    let mut stdout = stdout;
                    tokio::io::copy(&mut stdout, sink).await
                }
            };
            let copied = match copied {
                Ok(n) if cap.is_some_and(|c| n > c) => {
                    let _ = child.start_kill();
                    Err(failed(format!(
                        "{name}: output larger than {} bytes",
                        cap.unwrap_or(0)
                    )))
                }
                Ok(n) => Ok(n),
                Err(e) => {
                    let _ = child.start_kill();
                    Err(failed(format!("{name}: cannot read its output: {e}")))
                }
            };
            let status = child
                .wait()
                .await
                .map_err(|e| failed(format!("{name}: cannot wait for docker: {e}")));
            (copied, status)
        };
        let (copied, status) = match tokio::time::timeout(timeout, run).await {
            Ok(r) => r,
            Err(_) => {
                return Err(failed(format!(
                    "{name} timed out after {} s",
                    timeout.as_secs()
                )));
            }
        };
        let stderr = err_task.await.unwrap_or_default();
        let n = copied?;
        Ok((status?, stderr, n))
    }

    async fn exec(
        &self,
        name: &str,
        argv: Vec<OsString>,
        timeout: Duration,
        stdin: std::process::Stdio,
    ) -> RuntimeResult<Ran> {
        let mut stdout = Vec::new();
        let (status, stderr, _) = self
            .exec_into(name, argv, timeout, stdin, &mut stdout, Some(MAX_CAPTURE))
            .await?;
        Ok(Ran {
            success: status.success(),
            status,
            stdout,
            stderr,
        })
    }

    async fn run(
        &self,
        name: &str,
        argv: Vec<OsString>,
        timeout: Duration,
    ) -> RuntimeResult<Vec<u8>> {
        self.exec(name, argv, timeout, std::process::Stdio::null())
            .await?
            .ok(name)
    }

    /// Compose fails on a missing `--env-file`; create an empty one first.
    fn compose_ready(&self) -> RuntimeResult<()> {
        ensure_release_env(&self.release_env, None)
    }

    async fn compose(
        &self,
        name: &str,
        argv: Vec<OsString>,
        timeout: Duration,
    ) -> RuntimeResult<Vec<u8>> {
        self.compose_ready()?;
        self.run(name, argv, timeout).await
    }

    /// The server's health status, or why there is none.
    async fn server_health(&self, timeout: Duration) -> RuntimeResult<String> {
        let out = self
            .compose("docker compose ps", self.server_container_argv(), timeout)
            .await?;
        let text = String::from_utf8_lossy(&out);
        let Some(id) = text.lines().map(str::trim).find(|l| !l.is_empty()) else {
            return Err(failed("the server container is not running"));
        };
        let out = self
            .run("docker inspect", self.health_argv(id)?, timeout)
            .await?;
        Ok(String::from_utf8_lossy(&out).trim().to_string())
    }

    async fn cp_file(&self, id: &str, name: &str) -> RuntimeResult<Vec<u8>> {
        let tar = self
            .run(
                "docker cp",
                self.cp_argv(id, &format!("/{name}"))?,
                CMD_TIMEOUT,
            )
            .await?;
        extract_tar_file(&tar, name).map_err(failed)
    }
}

#[async_trait]
impl Runtime for DockerCli {
    async fn fetch_release(&self, selector: ReleaseSelector) -> RuntimeResult<(Vec<u8>, Vec<u8>)> {
        let r = self.release_ref(&selector)?;
        self.run("docker pull", self.release_pull_argv(&r), PULL_TIMEOUT)
            .await?;
        let out = self
            .run("docker create", self.release_create_argv(&r), CMD_TIMEOUT)
            .await?;
        let text = String::from_utf8_lossy(&out);
        let id = container_id(text.trim())?.to_string();
        let files = async {
            let manifest = self.cp_file(&id, "manifest.json").await?;
            let bundle = self.cp_file(&id, "manifest.sigstore.json").await?;
            Ok((manifest, bundle))
        }
        .await;
        if let Err(e) = self.run("docker rm", self.rm_argv(&id)?, CMD_TIMEOUT).await {
            tracing::warn!(container = %id, error = %e, "cannot remove the temporary release container");
        }
        files
    }

    async fn pull(&self, image: &str) -> RuntimeResult<()> {
        self.run("docker pull", self.pull_argv(image)?, PULL_TIMEOUT)
            .await
            .map(drop)
    }

    async fn image_labels(&self, image: &str) -> RuntimeResult<HashMap<String, String>> {
        let out = self
            .run(
                "docker image inspect",
                self.labels_argv(image)?,
                CMD_TIMEOUT,
            )
            .await?;
        let labels: Option<HashMap<String, String>> =
            serde_json::from_slice(&out).map_err(|e| {
                failed(format!(
                    "docker image inspect: unexpected labels output: {e}"
                ))
            })?;
        Ok(labels.unwrap_or_default())
    }

    async fn free_bytes(&self, path: &Path) -> RuntimeResult<u64> {
        let st = nix::sys::statvfs::statvfs(path)
            .map_err(|e| failed(format!("statvfs {}: {e}", path.display())))?;
        #[allow(clippy::unnecessary_cast)]
        Ok((st.blocks_available() as u64).saturating_mul(st.fragment_size() as u64))
    }

    async fn backup(&self, dest: &Path) -> RuntimeResult<u64> {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        use tokio::io::AsyncWriteExt;
        self.compose_ready()?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dest)
            .map_err(|e| failed(format!("cannot create backup {}: {e}", dest.display())))?;
        let result = async {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| failed(format!("cannot set the mode of {}: {e}", dest.display())))?;
            let mut out = tokio::fs::File::from_std(file);
            let (status, stderr, n) = self
                .exec_into(
                    "pg_dump",
                    self.backup_argv(),
                    BACKUP_TIMEOUT,
                    std::process::Stdio::null(),
                    &mut out,
                    None,
                )
                .await?;
            if !status.success() {
                return Err(failed(format!(
                    "pg_dump failed ({status}): {}",
                    stderr_tail(&stderr)
                )));
            }
            if n == 0 {
                return Err(failed("pg_dump produced no output"));
            }
            let sync = |e: std::io::Error| failed(format!("cannot sync {}: {e}", dest.display()));
            out.flush().await.map_err(sync)?;
            out.sync_all().await.map_err(sync)?;
            fsync_parent(dest).map_err(sync)?;
            Ok(n)
        }
        .await;
        if result.is_err() {
            // This call created the file (create_new): remove the partial dump.
            let _ = std::fs::remove_file(dest);
        }
        result
    }

    async fn restore(&self, src: &Path) -> RuntimeResult<()> {
        let [drop_db, create_db, pg_restore] = self.restore_argvs();
        // Opened first: a missing dump must not cost the database.
        let dump = std::fs::File::open(src)
            .map_err(|e| failed(format!("cannot open backup {}: {e}", src.display())))?;
        self.compose("psql (drop database)", drop_db, CMD_TIMEOUT)
            .await?;
        self.compose("psql (create database)", create_db, CMD_TIMEOUT)
            .await?;
        self.compose_ready()?;
        self.exec("pg_restore", pg_restore, BACKUP_TIMEOUT, dump.into())
            .await?
            .ok("pg_restore")
            .map(drop)
    }

    async fn start_server(&self, image: &str) -> RuntimeResult<()> {
        write_release_env(&self.release_env, image)?;
        self.compose("docker compose up", self.start_server_argv(), CMD_TIMEOUT)
            .await
            .map(drop)
    }

    async fn stop_server(&self) -> RuntimeResult<()> {
        self.compose("docker compose stop", self.stop_server_argv(), CMD_TIMEOUT)
            .await
            .map(drop)
    }

    async fn wait_healthy(&self, deadline: Duration) -> RuntimeResult<()> {
        let started = tokio::time::Instant::now();
        let mut last;
        loop {
            let left = deadline.saturating_sub(started.elapsed());
            let per_call = CMD_TIMEOUT.min(left.max(Duration::from_secs(1)));
            match self.server_health(per_call).await {
                Ok(s) if s == "healthy" => return Ok(()),
                Ok(s) => last = format!("health {s:?}"),
                Err(RuntimeError::Failed(e)) => last = e,
                Err(e) => return Err(e),
            }
            if started.elapsed() + HEALTH_POLL > deadline {
                return Err(failed(format!(
                    "the server was not healthy within {} s (last: {last})",
                    deadline.as_secs()
                )));
            }
            tokio::time::sleep(HEALTH_POLL).await;
        }
    }

    async fn readiness(&self) -> RuntimeResult<ReadinessReport> {
        self.compose_ready()?;
        let name = "aq-server readiness";
        let ran = self
            .exec(
                name,
                self.readiness_argv(),
                CMD_TIMEOUT,
                std::process::Stdio::null(),
            )
            .await?;
        // A not-ready server still prints its report (and exits 1); the
        // engine judges the report.
        match serde_json::from_slice::<ReadinessReport>(&ran.stdout) {
            Ok(r) => Ok(r),
            Err(e) if ran.success => Err(failed(format!("{name}: unexpected output: {e}"))),
            Err(_) => Err(failed(format!(
                "{name} failed ({}): {}",
                ran.status,
                stderr_tail(&ran.stderr)
            ))),
        }
    }

    async fn server_state_format(&self) -> RuntimeResult<Option<u32>> {
        self.compose_ready()?;
        let ran = self
            .exec(
                "psql (state format)",
                self.state_format_argv(),
                CMD_TIMEOUT,
                std::process::Stdio::null(),
            )
            .await?;
        parse_state_format(ran.success, &ran.stdout, &ran.stderr)
    }
}

/// The last 2 KiB of a child's stderr, for error messages.
pub fn stderr_tail(stderr: &[u8]) -> String {
    const MAX: usize = 2048;
    let mut tail = &stderr[stderr.len().saturating_sub(MAX)..];
    // Do not start in the middle of a UTF-8 sequence.
    while let [first, rest @ ..] = tail
        && (first & 0xC0) == 0x80
    {
        tail = rest;
    }
    let mut s = String::from_utf8_lossy(tail).trim().to_string();
    while s.len() > MAX {
        let mut cut = s.len() - MAX;
        while !s.is_char_boundary(cut) {
            cut += 1;
        }
        s.drain(..cut);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ComposeConfig;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    const IMAGE: &str = "ghcr.io/electricjack/agent-q-server@sha256:0000000000000000000000000000000000000000000000000000000000000100";

    fn cli() -> DockerCli {
        DockerCli::new(
            ComposeConfig {
                project_dir: "/opt/agent-q".into(),
                files: vec![
                    "/opt/agent-q/compose.yaml".into(),
                    "/opt/agent-q/compose.prod.yaml".into(),
                ],
                env_files: vec!["/etc/agent-q/compose.env".into()],
                project_name: "agent-q".into(),
            },
            Path::new("/var/lib/agent-q/updater"),
            "ghcr.io/electricjack/agent-q-release".into(),
            "stable".into(),
        )
    }

    fn strs(v: &[OsString]) -> Vec<&str> {
        v.iter().map(|a| a.to_str().unwrap()).collect()
    }

    const PREFIX: [&str; 13] = [
        "compose",
        "--project-name",
        "agent-q",
        "--project-directory",
        "/opt/agent-q",
        "-f",
        "/opt/agent-q/compose.yaml",
        "-f",
        "/opt/agent-q/compose.prod.yaml",
        "--env-file",
        "/etc/agent-q/compose.env",
        "--env-file",
        "/var/lib/agent-q/updater/release.env",
    ];

    fn with_prefix(rest: &[&'static str]) -> Vec<&'static str> {
        PREFIX.iter().chain(rest).copied().collect()
    }

    #[test]
    fn compose_args_are_fixed_vectors() {
        let c = cli();
        assert_eq!(
            c.release_env_path(),
            Path::new("/var/lib/agent-q/updater/release.env")
        );
        assert_eq!(
            strs(&c.start_server_argv()),
            with_prefix(&["up", "-d", "--no-deps", "--force-recreate", "server"])
        );
        assert_eq!(
            strs(&c.stop_server_argv()),
            with_prefix(&["stop", "server"])
        );
        assert_eq!(
            strs(&c.server_container_argv()),
            with_prefix(&["ps", "-q", "server"])
        );
        assert_eq!(
            strs(&c.readiness_argv()),
            with_prefix(&["exec", "-T", "server", "aq-server", "readiness"])
        );
        assert_eq!(
            strs(&c.backup_argv()),
            with_prefix(&[
                "exec", "-T", "postgres", "pg_dump", "-U", "agent_q", "-Fc", "agent_q"
            ])
        );
        let [drop, create, restore] = c.restore_argvs();
        assert_eq!(
            strs(&drop),
            with_prefix(&[
                "exec",
                "-T",
                "postgres",
                "psql",
                "-U",
                "agent_q",
                "-d",
                "postgres",
                "-c",
                "DROP DATABASE IF EXISTS agent_q WITH (FORCE)",
            ])
        );
        assert_eq!(
            strs(&create),
            with_prefix(&[
                "exec",
                "-T",
                "postgres",
                "psql",
                "-U",
                "agent_q",
                "-d",
                "postgres",
                "-c",
                "CREATE DATABASE agent_q OWNER agent_q",
            ])
        );
        assert_eq!(
            strs(&restore),
            with_prefix(&[
                "exec",
                "-T",
                "postgres",
                "pg_restore",
                "-U",
                "agent_q",
                "-d",
                "agent_q"
            ])
        );
        assert_eq!(
            strs(&c.state_format_argv()),
            with_prefix(&[
                "exec",
                "-T",
                "postgres",
                "psql",
                "-U",
                "agent_q",
                "-d",
                "agent_q",
                "-tAc",
                "SELECT value FROM meta WHERE key = 'state_format_version'",
            ])
        );
        // The server image never appears as an argument of a Compose call.
        for argv in [
            c.start_server_argv(),
            c.stop_server_argv(),
            c.readiness_argv(),
            c.backup_argv(),
            c.state_format_argv(),
        ] {
            assert!(
                strs(&argv).iter().all(|a| !a.contains("sha256")),
                "{argv:?}"
            );
        }
        // Without extra env files only the release env is passed.
        let mut bare = cli();
        bare.compose.env_files.clear();
        let argv = bare.stop_server_argv();
        let argv = strs(&argv);
        assert_eq!(argv.iter().filter(|a| **a == "--env-file").count(), 1);
    }

    #[test]
    fn docker_argvs_take_only_validated_refs() {
        let c = cli();
        assert_eq!(
            strs(&c.pull_argv(IMAGE).unwrap()),
            ["pull", "--quiet", IMAGE]
        );
        assert_eq!(
            strs(&c.labels_argv(IMAGE).unwrap()),
            [
                "image",
                "inspect",
                "--format",
                "{{json .Config.Labels}}",
                IMAGE
            ]
        );
        for bad in [
            "ghcr.io/electricjack/agent-q-server:latest",
            "--help",
            "ghcr.io/x@sha256:00",
            &format!("{IMAGE}\nAQ_OTHER=1"),
        ] {
            assert!(c.pull_argv(bad).is_err(), "{bad}");
            assert!(c.labels_argv(bad).is_err(), "{bad}");
        }
        let v = ReleaseSelector::Version("1.2.3".parse().unwrap());
        assert_eq!(
            c.release_ref(&v).unwrap(),
            "ghcr.io/electricjack/agent-q-release:1.2.3"
        );
        assert_eq!(
            c.release_ref(&ReleaseSelector::Channel).unwrap(),
            "ghcr.io/electricjack/agent-q-release:stable"
        );
        // Build metadata is not a valid tag character.
        let v = ReleaseSelector::Version("1.2.3+x".parse().unwrap());
        assert!(c.release_ref(&v).is_err());
        let mut odd = cli();
        odd.channel = "-bad".into();
        assert!(odd.release_ref(&ReleaseSelector::Channel).is_err());

        let id = "0123456789ab";
        assert_eq!(
            strs(&c.release_create_argv("ghcr.io/electricjack/agent-q-release:1.2.3")),
            [
                "create",
                "ghcr.io/electricjack/agent-q-release:1.2.3",
                "none"
            ]
        );
        assert_eq!(
            strs(&c.cp_argv(id, "/manifest.json").unwrap()),
            ["cp", "0123456789ab:/manifest.json", "-"]
        );
        assert_eq!(strs(&c.rm_argv(id).unwrap()), ["rm", id]);
        assert_eq!(
            strs(&c.health_argv(id).unwrap()),
            ["inspect", "--format", "{{.State.Health.Status}}", id]
        );
        for bad in [
            "",
            "abc",
            "0123456789AB",
            "0123456789ab:/etc/passwd",
            "--rm",
        ] {
            assert!(c.cp_argv(bad, "/manifest.json").is_err(), "{bad}");
            assert!(c.rm_argv(bad).is_err(), "{bad}");
            assert!(c.health_argv(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn release_env_written_atomically_with_mode_0640() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("release.env");
        std::fs::write(t.path().join("release.env.tmp"), "stale").unwrap();
        write_release_env(&path, IMAGE).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("AQ_SERVER_IMAGE={IMAGE}\n")
        );
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o640, "{mode:o}");
        let names: Vec<_> = std::fs::read_dir(t.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![OsString::from("release.env")]);
        // Only digest refs are written: no tags, no injected lines.
        for bad in ["nginx:latest", &format!("{IMAGE}\nAQ_VAULT_DIR=/"), ""] {
            assert!(write_release_env(&path, bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("AQ_SERVER_IMAGE={IMAGE}\n")
        );
        // ensure_release_env never replaces an existing file.
        ensure_release_env(&path, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("AQ_SERVER_IMAGE={IMAGE}\n")
        );
        let fresh = t.path().join("sub-release.env");
        ensure_release_env(&fresh, Some(IMAGE)).unwrap();
        assert_eq!(
            std::fs::read_to_string(&fresh).unwrap(),
            format!("AQ_SERVER_IMAGE={IMAGE}\n")
        );
        let empty = t.path().join("empty-release.env");
        ensure_release_env(&empty, None).unwrap();
        assert_eq!(std::fs::read_to_string(&empty).unwrap(), "");
        let mode = std::fs::metadata(&empty).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o640, "{mode:o}");
    }

    fn tar_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        b.into_inner().unwrap()
    }

    #[test]
    fn extracts_the_named_file_from_a_docker_cp_tar() {
        let t = tar_of(&[("manifest.json", b"{\"a\":1}")]);
        assert_eq!(extract_tar_file(&t, "manifest.json").unwrap(), b"{\"a\":1}");
        // Missing or wrongly named entries fail.
        assert!(extract_tar_file(&t, "manifest.sigstore.json").is_err());
        assert!(extract_tar_file(b"not a tar", "manifest.json").is_err());
        // A symlink is not a file.
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        b.append_link(&mut h, "manifest.json", "/etc/passwd")
            .unwrap();
        let link = b.into_inner().unwrap();
        assert!(extract_tar_file(&link, "manifest.json").is_err());
        // Oversized content is refused.
        let big = vec![b'x'; MAX_RELEASE_FILE + 1];
        let t = tar_of(&[("manifest.json", &big)]);
        assert!(extract_tar_file(&t, "manifest.json").is_err());
    }

    #[test]
    fn missing_database_or_table_is_no_state_format() {
        assert_eq!(parse_state_format(true, b"3\n", b"").unwrap(), Some(3));
        assert_eq!(parse_state_format(true, b"", b"").unwrap(), None);
        assert_eq!(
            parse_state_format(false, b"", b"psql: error: connection to server on socket \"/var/run/postgresql/.s.PGSQL.5432\" failed: FATAL:  database \"agent_q\" does not exist").unwrap(),
            None
        );
        assert_eq!(
            parse_state_format(
                false,
                b"",
                b"ERROR:  relation \"meta\" does not exist\nLINE 1: SELECT value FROM meta"
            )
            .unwrap(),
            None
        );
        assert!(parse_state_format(false, b"", b"service \"postgres\" is not running").is_err());
        assert!(parse_state_format(true, b"three", b"").is_err());
    }

    #[tokio::test]
    async fn failures_name_the_command_and_keep_stderr() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let fake = t.path().join("docker");
        std::fs::write(&fake, "#!/bin/sh\necho \"boom: $1\" >&2\nexit 7\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let c = cli().with_docker_binary(fake.clone());
        let e = c.pull(IMAGE).await.unwrap_err();
        let RuntimeError::Failed(m) = e else {
            panic!("{e:?}")
        };
        assert!(m.contains("docker pull") && m.contains("boom: pull"), "{m}");
        let e = c.pull("nginx:latest").await.unwrap_err();
        assert!(matches!(e, RuntimeError::Failed(_)), "{e:?}");

        // backup streams stdout into a new 0600 file and fsyncs it.
        std::fs::write(&fake, "#!/bin/sh\nprintf 'PGDMP-data'\n").unwrap();
        let dest = t.path().join("b.dump");
        let c = DockerCli {
            release_env: t.path().join("release.env"),
            ..cli().with_docker_binary(fake.clone())
        };
        assert_eq!(c.backup(&dest).await.unwrap(), 10);
        assert_eq!(std::fs::read(&dest).unwrap(), b"PGDMP-data");
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{mode:o}");
        // ...and never touches an existing file.
        assert!(c.backup(&dest).await.is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"PGDMP-data");
        // A failing dump removes the partial file it created.
        std::fs::write(
            &fake,
            "#!/bin/sh\nprintf 'partial'\necho 'disk full' >&2\nexit 1\n",
        )
        .unwrap();
        let dest2 = t.path().join("c.dump");
        let e = c.backup(&dest2).await.unwrap_err();
        assert!(
            e.to_string().contains("pg_dump") && e.to_string().contains("disk full"),
            "{e}"
        );
        assert!(!dest2.exists());
    }

    #[tokio::test]
    async fn fetch_release_removes_the_container_even_on_error() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let fake = t.path().join("docker");
        let log = t.path().join("log");
        // create prints an id; cp fails; every call is logged.
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\necho \"$*\" >> {log}\ncase \"$1\" in\n  create) echo 0123456789abcdef ;;\n  cp) echo 'no such file' >&2; exit 1 ;;\nesac\n",
                log = log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let c = cli().with_docker_binary(fake);
        let e = c.fetch_release(ReleaseSelector::Channel).await.unwrap_err();
        assert!(e.to_string().contains("docker cp"), "{e}");
        let calls = std::fs::read_to_string(&log).unwrap();
        let calls: Vec<&str> = calls.lines().collect();
        assert_eq!(
            calls,
            [
                "pull --quiet ghcr.io/electricjack/agent-q-release:stable",
                "create ghcr.io/electricjack/agent-q-release:stable none",
                "cp 0123456789abcdef:/manifest.json -",
                "rm 0123456789abcdef",
            ]
        );
    }

    #[test]
    fn release_env_path_is_under_the_state_dir() {
        let c = cli();
        assert_eq!(
            c.release_env_path(),
            PathBuf::from("/var/lib/agent-q/updater/release.env")
        );
    }

    #[test]
    fn stderr_tail_is_bounded_and_keeps_the_end() {
        let mut long = vec![b'a'; 5000];
        long.extend_from_slice(b"the end\n");
        let t = stderr_tail(&long);
        assert!(t.len() <= 2048);
        assert!(t.ends_with("the end"));
        // A multi-byte character cut at the boundary is dropped, not mangled.
        let mut s = "é".repeat(2000).into_bytes();
        s.push(b'!');
        let t = stderr_tail(&s);
        assert!(t.len() <= 2048 && t.ends_with('!') && !t.contains('\u{FFFD}'));
    }
}
