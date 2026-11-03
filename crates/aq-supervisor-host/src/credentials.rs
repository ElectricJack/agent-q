//! Bounded retained credential placement. Errors and Debug never expose bytes.
use anyhow::{Result, ensure};
use aq_responses_transport::RetainedUpstreamCredential;
use aq_session_protocol::Digest;
use std::{
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
use zeroize::Zeroizing;

pub struct SecretFile {
    path: PathBuf,
    digest: Digest,
    device: u64,
    inode: u64,
}
impl std::fmt::Debug for SecretFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretFile([retained])")
    }
}
fn read(path: &std::path::Path) -> Result<(Zeroizing<Vec<u8>>, u64, u64)> {
    ensure!(
        path.is_absolute() && path.canonicalize()? == path,
        "credential placement rejected"
    );
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o600
            && metadata.nlink() == 1
            && metadata.len() <= 4096,
        "credential file rejected"
    );
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(4097).read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= 4096,
        "credential size rejected"
    );
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    ensure!(
        !bytes.is_empty() && bytes.iter().all(u8::is_ascii_graphic),
        "credential encoding rejected"
    );
    Ok((bytes, metadata.dev(), metadata.ino()))
}
impl SecretFile {
    pub fn open(path: PathBuf) -> Result<Self> {
        let (bytes, device, inode) =
            read(&path).map_err(|_| anyhow::anyhow!("credential placement unavailable"))?;
        Ok(Self {
            path,
            digest: Digest::of(&bytes),
            device,
            inode,
        })
    }
}
impl RetainedUpstreamCredential for SecretFile {
    fn read_current(&self) -> Result<Zeroizing<Vec<u8>>> {
        let (bytes, device, inode) =
            read(&self.path).map_err(|_| anyhow::anyhow!("credential placement unavailable"))?;
        ensure!(
            self.device == device && self.inode == inode && self.digest == Digest::of(&bytes),
            "credential placement changed"
        );
        Ok(bytes)
    }
}
