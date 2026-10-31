//! Private configured-file owner. No caller path, URL, command or secret value
//! enters a lease request. Read outside SQL locks; the caller must subsequently
//! fence current assignment, registry revision, lease and expiry before delivery.
use crate::intelligence::config::LoadedRegistry;
use aq_admission::{MaterialLease, SessionIdentity};
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MaterialError {
    #[error("configured material owner unavailable")]
    Unavailable,
    #[error("material scope is not authorized")]
    Scope,
    #[error("configured material changed; registry reload required")]
    Changed,
}
/// Intentionally no Debug, Serialize or Clone. Only the authenticated private
/// response producer may borrow bytes; zeroization covers this owner buffer,
/// not copies owned by the HTTP stack or the remote receiving process.
pub struct ProtectedMaterial(Zeroizing<Vec<u8>>);
impl ProtectedMaterial {
    pub fn bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

pub fn read_for_lease(
    registry: &LoadedRegistry,
    lease: &MaterialLease,
    node_id: Uuid,
) -> Result<ProtectedMaterial, MaterialError> {
    lease.validate().map_err(|_| MaterialError::Scope)?;
    if !matches!(lease.binding.session,SessionIdentity::Remote {node_id:assigned,..} if assigned==node_id)
    {
        return Err(MaterialError::Scope);
    }
    let (index, credential) = registry
        .credentials
        .iter()
        .enumerate()
        .find(|(_, credential)| {
            credential.profile == lease.credential_profile
                && credential.revision == lease.credential_revision
        })
        .ok_or(MaterialError::Scope)?;
    let configured = registry
        .config
        .credentials
        .get(index)
        .ok_or(MaterialError::Unavailable)?;
    if !credential.enabled
        || !credential.provisionable
        || credential.material_owner.is_none()
        || !configured.allowed_node_ids.contains(&node_id)
        || credential.provider != lease.provider.as_str()
        || credential.account_reference != lease.account_reference.as_str()
        || credential.endpoint_reference != lease.endpoint_reference.as_str()
    {
        return Err(MaterialError::Scope);
    }
    let bytes = read_configured(registry, index)?;
    if aq_admission::Digest::of(bytes.bytes()) != lease.material_digest
        || bytes.bytes().len() as u64 != lease.material_bytes
    {
        return Err(MaterialError::Changed);
    }
    Ok(bytes)
}

fn read_configured(
    registry: &LoadedRegistry,
    index: usize,
) -> Result<ProtectedMaterial, MaterialError> {
    let expected = registry
        .fingerprints
        .get(index)
        .and_then(Option::as_ref)
        .ok_or(MaterialError::Unavailable)?;
    let path = registry
        .config
        .credentials
        .get(index)
        .and_then(|c| c.secret_file.as_ref())
        .ok_or(MaterialError::Unavailable)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| MaterialError::Unavailable)?;
    let metadata = file.metadata().map_err(|_| MaterialError::Unavailable)?;
    if !metadata.is_file()
        || metadata.mode() & 0o077 != 0
        || metadata.uid() != nix::unistd::geteuid().as_raw()
    {
        return Err(MaterialError::Unavailable);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| MaterialError::Unavailable)?;
    if bytes.is_empty() || bytes.len() > 65_536 {
        return Err(MaterialError::Unavailable);
    }
    if aq_admission::Digest::of(&bytes).as_str() != expected {
        return Err(MaterialError::Changed);
    }
    Ok(ProtectedMaterial(bytes))
}
/// Trusted daemon facts only. Call outside SQL locks, then fence the exact
/// admitted runtime, registry and observation age in the write transaction.
/// File presence yields preparation readiness, never installed availability.
pub(crate) fn provisioning_bindings(
    registry: &LoadedRegistry,
    node: Uuid,
    boot: Uuid,
    delivery: &aq_selection::DeliveryCapability,
    observed: u64,
    expires: u64,
) -> Vec<aq_selection::CredentialBinding> {
    if node.is_nil()
        || boot.is_nil()
        || observed == 0
        || expires <= observed
        || expires - observed > 120_000
        || delivery.version != 1
    {
        return vec![];
    }
    registry
        .credentials
        .iter()
        .enumerate()
        .filter_map(|(index, c)| {
            let cfg = registry.config.credentials.get(index)?;
            let owner = c.material_owner.as_ref()?;
            if !c.enabled
                || !c.provisionable
                || cfg.delivery_mode.is_none()
                || !cfg.allowed_node_ids.contains(&node)
                || read_configured(registry, index).is_err()
            {
                return None;
            }
            Some(aq_selection::CredentialBinding {
                credential_revision: c.revision,
                credential_profile: c.profile.clone(),
                available: false,
                preparation: Some(aq_selection::ProvisioningReadiness {
                    owner: owner.clone(),
                    credential_profile: c.profile.clone(),
                    credential_revision: c.revision,
                    target: aq_selection::ExecutionTarget::Worker {
                        node_id: node,
                        boot_id: boot,
                    },
                    delivery: delivery.clone(),
                    observed_at_ms: observed,
                    expires_at_ms: expires,
                }),
                observed_at_ms: observed,
                expires_at_ms: expires,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::config::{CredentialConfig, IntelligenceConfig, MaterialDeliveryMode};
    use std::os::unix::fs::PermissionsExt;
    fn lease() -> MaterialLease {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../aq-admission/artifacts/fixtures.json"
        ))
        .unwrap();
        serde_json::from_value(
            fixtures
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["kind"] == "material_lease")
                .unwrap()["body"]
                .clone(),
        )
        .unwrap()
    }
    fn config(path: std::path::PathBuf, lease: &MaterialLease, node: Uuid) -> IntelligenceConfig {
        IntelligenceConfig {
            revision: 1,
            credentials: vec![CredentialConfig {
                credential: aq_selection::AuthorizedCredential {
                    profile: lease.credential_profile.clone(),
                    provider: lease.provider.as_str().into(),
                    account_reference: lease.account_reference.as_str().into(),
                    endpoint_reference: lease.endpoint_reference.as_str().into(),
                    revision: lease.credential_revision,
                    enabled: true,
                    provisionable: false,
                    material_owner: None,
                },
                allowed_node_ids: vec![node],
                secret_file: Some(path),
                delivery_mode: Some(MaterialDeliveryMode::ServerMaterialV1),
            }],
            qualified_harnesses: vec![],
        }
    }
    #[test]
    fn delivery_requires_exact_scope_and_current_bounded_file_bytes() {
        let lease = lease();
        let SessionIdentity::Remote { node_id, .. } = lease.binding.session else {
            unreachable!()
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private-key");
        std::fs::write(&path, b"synthetic-key").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let registry = config(path.clone(), &lease, node_id).load().unwrap();
        assert_eq!(
            read_for_lease(&registry, &lease, node_id).unwrap().bytes(),
            b"synthetic-key"
        );
        assert!(matches!(
            read_for_lease(&registry, &lease, Uuid::new_v4()),
            Err(MaterialError::Scope)
        ));
        let mut wrong = lease.clone();
        wrong.account_reference = aq_admission::Label::new("other-account").unwrap();
        assert!(matches!(
            read_for_lease(&registry, &wrong, node_id),
            Err(MaterialError::Scope)
        ));
        std::fs::write(&path, b"different-key").unwrap();
        assert!(matches!(
            read_for_lease(&registry, &lease, node_id),
            Err(MaterialError::Changed)
        ));
        std::fs::write(&path, vec![b'x'; 65_537]).unwrap();
        assert!(matches!(
            read_for_lease(&registry, &lease, node_id),
            Err(MaterialError::Unavailable)
        ));
        std::fs::write(&path, b"synthetic-key").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            read_for_lease(&registry, &lease, node_id),
            Err(MaterialError::Unavailable)
        ));
        std::fs::remove_file(&path).unwrap();
        nix::unistd::mkfifo(
            &path,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        assert!(matches!(
            read_for_lease(&registry, &lease, node_id),
            Err(MaterialError::Unavailable)
        ));
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("missing", &path).unwrap();
        assert!(matches!(
            read_for_lease(&registry, &lease, node_id),
            Err(MaterialError::Unavailable)
        ));
        let mut installed = config(path, &lease, node_id);
        installed.credentials[0].delivery_mode = None;
        installed.credentials[0].secret_file = None;
        assert!(matches!(
            read_for_lease(&installed.load().unwrap(), &lease, node_id),
            Err(MaterialError::Scope)
        ));
    }
}

mod issuance;
pub use issuance::issue_for_selection;
