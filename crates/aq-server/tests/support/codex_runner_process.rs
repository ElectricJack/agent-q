//! Fixture process/pipe owner around the existing confined ordinary CLI. It
//! never creates admission, launch, stopped, source or result authority.
#![allow(dead_code)]
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::Path,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
};
pub struct RunnerProcess {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl RunnerProcess {
    pub fn start(
        wrapper: &Path,
        environment: &BTreeMap<String, String>,
        stderr: &Path,
    ) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let error = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(stderr)?;
        let mut child = tokio::process::Command::new("/bin/bash")
            .arg(wrapper)
            .arg("--runner")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .envs(environment)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(error))
            .process_group(0)
            .kill_on_drop(true)
            .spawn()?;
        let input = child
            .stdin
            .take()
            .context("fixture ordinary CLI input absent")?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .context("fixture ordinary CLI output absent")?,
        );
        Ok(Self {
            child,
            input: Some(input),
            output,
        })
    }
    pub fn pid(&self) -> Result<u32> {
        self.child.id().context("owned wrapper already reaped")
    }
    pub async fn preflight(&mut self) -> Result<Value> {
        tokio::time::timeout(Duration::from_secs(45), async {
            for _ in 0..4 {
                let mut line = String::new();
                let bytes = (&mut self.output).take(65537).read_line(&mut line).await?;
                ensure!(
                    bytes > 0 && bytes <= 65536,
                    "bounded native preflight frame"
                );
                if line.starts_with("qualification guard: ") {
                    continue;
                }
                let ready: Value =
                    serde_json::from_str(&line).context("closed native preflight frame")?;
                ensure!(
                    ready["schema_version"] == 1
                        && ready["phase"] == "native_preflight_ready"
                        && ready["worker_version"] == "0.5.0",
                    "actual current preflight required"
                );
                return Ok::<_, anyhow::Error>(ready);
            }
            anyhow::bail!("native preflight frame absent")
        })
        .await
        .context("native preflight outer bound")?
    }
    pub async fn continue_once(&mut self) -> Result<()> {
        let mut input = self
            .input
            .take()
            .context("fixture continuation already consumed")?;
        tokio::time::timeout(Duration::from_secs(1), async {
            input
                .write_all(b"{\"schema_version\":1,\"continue\":true}\n")
                .await?;
            input.flush().await?;
            Ok::<_, std::io::Error>(())
        })
        .await
        .context("bounded fixture continuation")??;
        drop(input); // Actual pipe ownership closes here, including on errors.
        Ok(())
    }
    pub async fn wait(&mut self) -> Result<ExitStatus> {
        // Outer teardown bound only. The ordinary driver and guardian continue
        // enforcing the original20s grant; post-stop capture/B1 is separate.
        tokio::time::timeout(Duration::from_secs(55), self.child.wait())
            .await
            .context("ordinary CLI finalization bound")?
            .context("ordinary CLI wait")
    }
    pub async fn cleanup(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.child.try_wait()? {
            return Ok(status);
        }
        let pid = self.pid()?;
        // Child stays unreaped while its original PGID is signaled. This is
        // fixture teardown, never an AQ ECHILD/stopped proof. The private PID
        // namespace init dies with the wrapper; its exact peer pidfd is also
        // retained by the separate transfer owner for exit observation.
        unsafe {
            nix::libc::kill(-(pid as i32), nix::libc::SIGTERM);
        }
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await {
            return Ok(result?);
        }
        unsafe {
            nix::libc::kill(-(pid as i32), nix::libc::SIGKILL);
        }
        tokio::time::timeout(Duration::from_secs(3), self.child.wait())
            .await
            .context("wrapper cleanup remains unknown")?
            .context("wrapper cleanup wait")
    }
}
