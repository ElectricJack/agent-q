//! Release authenticity. Production uses cosign keyless verification against
//! the release workflow's identity; the ed25519 `TestKey` exists only with
//! the `test-verifier` feature.

use crate::config::VerifierConfig;
use crate::runtime::stderr_tail;
use async_trait::async_trait;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

/// How long `cosign verify-blob` may run before it is killed.
pub const COSIGN_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    /// The signature does not authenticate the manifest.
    #[error("release signature rejected: {0}")]
    Rejected(String),
    /// The verifier could not run (missing binary, bad key configuration).
    #[error("release verifier unavailable: {0}")]
    Unavailable(String),
}

#[async_trait]
pub trait ReleaseVerifier: Send + Sync {
    /// Check that `bundle` authenticates exactly these `manifest` bytes.
    async fn verify(&self, manifest: &[u8], bundle: &[u8]) -> Result<(), VerifyError>;
}

#[async_trait]
impl ReleaseVerifier for Box<dyn ReleaseVerifier> {
    async fn verify(&self, manifest: &[u8], bundle: &[u8]) -> Result<(), VerifyError> {
        (**self).verify(manifest, bundle).await
    }
}

/// Build the verifier the config names.
pub fn from_config(cfg: &VerifierConfig) -> Result<Box<dyn ReleaseVerifier>, VerifyError> {
    match cfg {
        VerifierConfig::CosignKeyless {
            cosign_path,
            identity_regexp,
            oidc_issuer,
        } => Ok(Box::new(CosignKeyless::new(
            cosign_path.clone(),
            identity_regexp.clone(),
            oidc_issuer.clone(),
        ))),
        #[cfg(feature = "test-verifier")]
        VerifierConfig::TestKey { public_key_hex } => {
            Ok(Box::new(TestKey::from_hex(public_key_hex)?))
        }
    }
}

/// `cosign verify-blob` with a fixed argument vector; no shell.
#[derive(Debug, Clone)]
pub struct CosignKeyless {
    cosign_path: PathBuf,
    identity_regexp: String,
    oidc_issuer: String,
    timeout: Duration,
}

impl CosignKeyless {
    pub fn new(cosign_path: PathBuf, identity_regexp: String, oidc_issuer: String) -> Self {
        CosignKeyless {
            cosign_path,
            identity_regexp,
            oidc_issuer,
            timeout: COSIGN_TIMEOUT,
        }
    }

    fn args(&self, bundle: &Path, manifest: &Path) -> Vec<OsString> {
        vec![
            "verify-blob".into(),
            "--bundle".into(),
            bundle.into(),
            "--certificate-identity-regexp".into(),
            self.identity_regexp.clone().into(),
            "--certificate-oidc-issuer".into(),
            self.oidc_issuer.clone().into(),
            manifest.into(),
        ]
    }
}

#[async_trait]
impl ReleaseVerifier for CosignKeyless {
    async fn verify(&self, manifest: &[u8], bundle: &[u8]) -> Result<(), VerifyError> {
        let unavailable =
            |what: &str, e: std::io::Error| VerifyError::Unavailable(format!("{what}: {e}"));
        let dir =
            tempfile::TempDir::new().map_err(|e| unavailable("cannot create a temp dir", e))?;
        let m = dir.path().join("manifest.json");
        let b = dir.path().join("manifest.sigstore.json");
        std::fs::write(&m, manifest).map_err(|e| unavailable("cannot write the manifest", e))?;
        std::fs::write(&b, bundle).map_err(|e| unavailable("cannot write the bundle", e))?;
        // kill_on_drop: when the timeout drops the wait, the child is killed.
        let child = tokio::process::Command::new(&self.cosign_path)
            .args(self.args(&b, &m))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| unavailable(&format!("cannot run {}", self.cosign_path.display()), e))?;
        let out = match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(r) => r.map_err(|e| unavailable("cannot wait for cosign", e))?,
            Err(_) => return Err(VerifyError::Unavailable("cosign timed out".into())),
        };
        if out.status.success() {
            Ok(())
        } else {
            Err(VerifyError::Rejected(format!(
                "cosign verify-blob exited with {}: {}",
                out.status,
                stderr_tail(&out.stderr)
            )))
        }
    }
}

/// Test-only trust root: the bundle is the lowercase hex of the 64-byte
/// ed25519 signature over the exact manifest bytes, with an optional
/// trailing newline.
#[cfg(feature = "test-verifier")]
#[derive(Debug, Clone)]
pub struct TestKey {
    key: ed25519_dalek::VerifyingKey,
}

#[cfg(feature = "test-verifier")]
impl TestKey {
    /// `public_key_hex`: 64 lowercase hex characters (32 bytes).
    pub fn from_hex(public_key_hex: &str) -> Result<Self, VerifyError> {
        let bytes: [u8; 32] = decode_lower_hex(public_key_hex.as_bytes()).ok_or_else(|| {
            VerifyError::Unavailable("test public key must be 64 lowercase hex characters".into())
        })?;
        let key = ed25519_dalek::VerifyingKey::from_bytes(&bytes)
            .map_err(|e| VerifyError::Unavailable(format!("invalid test public key: {e}")))?;
        Ok(TestKey { key })
    }
}

#[cfg(feature = "test-verifier")]
#[async_trait]
impl ReleaseVerifier for TestKey {
    async fn verify(&self, manifest: &[u8], bundle: &[u8]) -> Result<(), VerifyError> {
        let text = bundle.strip_suffix(b"\n").unwrap_or(bundle);
        let sig: [u8; 64] = decode_lower_hex(text).ok_or_else(|| {
            VerifyError::Rejected("bundle is not 128 lowercase hex characters".into())
        })?;
        self.key
            .verify_strict(manifest, &ed25519_dalek::Signature::from_bytes(&sig))
            .map_err(|_| {
                VerifyError::Rejected("ed25519 signature does not match the manifest".into())
            })
    }
}

#[cfg(feature = "test-verifier")]
fn decode_lower_hex<const N: usize>(s: &[u8]) -> Option<[u8; N]> {
    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    }
    if s.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (o, pair) in out.iter_mut().zip(s.chunks_exact(2)) {
        *o = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cosign(path: &str) -> CosignKeyless {
        CosignKeyless::new(
            path.into(),
            r"^https://github\.com/ElectricJack/agent-q/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$".into(),
            "https://token.actions.githubusercontent.com".into(),
        )
    }

    #[test]
    fn cosign_argv_is_fixed() {
        let c = cosign("/usr/local/bin/cosign");
        let args = c.args(Path::new("/t/b.json"), Path::new("/t/m.json"));
        let args: Vec<&str> = args.iter().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(
            args,
            [
                "verify-blob",
                "--bundle",
                "/t/b.json",
                "--certificate-identity-regexp",
                r"^https://github\.com/ElectricJack/agent-q/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$",
                "--certificate-oidc-issuer",
                "https://token.actions.githubusercontent.com",
                "/t/m.json",
            ]
        );
    }

    #[tokio::test]
    async fn cosign_missing_binary_is_unavailable() {
        let e = cosign("/nonexistent/cosign")
            .verify(b"{}", b"{}")
            .await
            .unwrap_err();
        assert!(matches!(e, VerifyError::Unavailable(_)), "{e}");
    }

    #[tokio::test]
    async fn cosign_nonzero_exit_is_rejected() {
        // `false` ignores its arguments and exits 1; no real cosign runs.
        let Some(f) = ["/bin/false", "/usr/bin/false"]
            .into_iter()
            .find(|p| Path::new(p).exists())
        else {
            return;
        };
        let e = cosign(f).verify(b"{}", b"{}").await.unwrap_err();
        assert!(matches!(e, VerifyError::Rejected(_)), "{e}");
    }

    #[tokio::test]
    async fn cosign_timeout_kills_and_is_unavailable() {
        use std::os::unix::fs::PermissionsExt;
        // A stand-in that ignores its arguments and hangs.
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("hang");
        let pidfile = dir.path().join("pid");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho $$ > {}\nexec sleep 30\n",
                pidfile.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut c = cosign(script.to_str().unwrap());
        c.timeout = Duration::from_millis(300);
        let started = std::time::Instant::now();
        let e = c.verify(b"{}", b"{}").await.unwrap_err();
        assert_eq!(e, VerifyError::Unavailable("cosign timed out".into()));
        assert!(started.elapsed() < Duration::from_secs(10));
        // The child was killed, not left running.
        let pid = std::fs::read_to_string(&pidfile).unwrap();
        let proc_dir = format!("/proc/{}", pid.trim());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while Path::new(&proc_dir).exists()
            && !std::fs::read_to_string(format!("{proc_dir}/stat"))
                .unwrap_or_default()
                .contains(") Z")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "cosign child still running"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[cfg(feature = "test-verifier")]
    mod test_key {
        use super::super::*;
        use ed25519_dalek::{Signer, SigningKey};

        fn hex(b: &[u8]) -> String {
            b.iter().map(|x| format!("{x:02x}")).collect()
        }

        fn pair(seed: u8) -> (SigningKey, TestKey) {
            let sk = SigningKey::from_bytes(&[seed; 32]);
            let tk = TestKey::from_hex(&hex(sk.verifying_key().as_bytes())).unwrap();
            (sk, tk)
        }

        #[tokio::test]
        async fn accepts_signature_with_or_without_newline() {
            let (sk, tk) = pair(7);
            let sig = hex(&sk.sign(b"manifest").to_bytes());
            tk.verify(b"manifest", sig.as_bytes()).await.unwrap();
            tk.verify(b"manifest", format!("{sig}\n").as_bytes())
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn rejects_tampering_wrong_key_and_bad_encoding() {
            let (sk, tk) = pair(7);
            let (_, other) = pair(8);
            let sig = hex(&sk.sign(b"manifest").to_bytes());
            let upper = sig.to_uppercase();
            let doubled = format!("{sig}\n\n");
            let cases: [(&TestKey, &[u8], &[u8]); 6] = [
                (&tk, b"manifest!", sig.as_bytes()),
                (&other, b"manifest", sig.as_bytes()),
                (&tk, b"manifest", upper.as_bytes()),
                (&tk, b"manifest", doubled.as_bytes()),
                (&tk, b"manifest", &sig.as_bytes()[2..]),
                (&tk, b"manifest", b""),
            ];
            for (i, (key, m, b)) in cases.into_iter().enumerate() {
                let r = key.verify(m, b).await;
                assert!(
                    matches!(r, Err(VerifyError::Rejected(_))),
                    "case {i}: {r:?}"
                );
            }
        }

        #[test]
        fn public_key_must_be_64_lowercase_hex() {
            let (sk, _) = pair(7);
            let h = hex(sk.verifying_key().as_bytes());
            assert!(TestKey::from_hex(&h).is_ok());
            for bad in [
                h.to_uppercase(),
                h[2..].to_string(),
                format!("{h}00"),
                "zz".repeat(32),
            ] {
                assert!(
                    matches!(TestKey::from_hex(&bad), Err(VerifyError::Unavailable(_))),
                    "{bad}"
                );
            }
        }
    }
}
