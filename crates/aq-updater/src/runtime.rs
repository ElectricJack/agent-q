//! The boundary between the update engine and the host: Docker, Compose and
//! the database container. Only the trait lives here for now; the `docker`
//! CLI implementation arrives with the CLI.

use aq_release::ReadinessReport;
use async_trait::async_trait;
use std::collections::HashMap;
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
    use super::stderr_tail;

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
