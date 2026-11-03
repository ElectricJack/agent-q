use aq_responses_transport::RetainedUpstreamCredential;
use aq_supervisor_host::credentials::SecretFile;
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn credential_reader_refuses_unsafe_files_and_rotation_without_disclosure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("provider");
    std::fs::write(&path, b"test-provider-secret").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let source = SecretFile::open(path.clone()).unwrap();
    assert_eq!(&*source.read_current().unwrap(), b"test-provider-secret");
    assert!(!format!("{source:?}").contains("test-provider-secret"));
    std::fs::write(&path, b"rotated-provider-secret").unwrap();
    let error = source.read_current().unwrap_err().to_string();
    assert!(!error.contains("rotated-provider-secret"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(SecretFile::open(path.clone()).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.path().join("link");
    symlink(&path, &link).unwrap();
    assert!(SecretFile::open(link).is_err());
    std::fs::write(&path, vec![b'a'; 4097]).unwrap();
    assert!(SecretFile::open(path).is_err());
}
