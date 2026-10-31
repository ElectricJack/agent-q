use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActivationState {
    pub revision: u64,
    pub active_digest: Option<String>,
    /// Durable identity tombstones prevent reusing id/version with changed bytes.
    pub known_identities: BTreeMap<String, String>,
    /// Exact attempt/retained-record reference -> immutable catalog digest.
    pub retained: BTreeMap<String, String>,
}
impl ActivationState {
    pub fn prepare_activation(
        &self,
        expected_revision: u64,
        catalog: &ValidatedCatalog,
    ) -> Result<Self> {
        if self.revision != expected_revision {
            return Err(Error::RevisionConflict);
        }
        let mut next = self.clone();
        for (id, digest) in &catalog.artifact().identities {
            if self
                .known_identities
                .get(id)
                .is_some_and(|old| old != digest)
            {
                return Err(Error::IdentityConflict(id.clone()));
            }
            next.known_identities.insert(id.clone(), digest.clone());
        }
        ensure(
            next.known_identities.len() <= 100_000,
            "activated identity history bound",
        )?;
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("activation revision exhausted".into()))?;
        next.active_digest = Some(catalog.digest().into());
        Ok(next)
    }
    pub fn can_remove(&self, digest: &str) -> bool {
        self.active_digest.as_deref() != Some(digest)
            && !self.retained.values().any(|d| d == digest)
    }
}
/// Trusted persistence boundary. Production adapters must serialize CAS, artifact
/// identity immutability and retain/release updates in their owning transaction.
/// No caller principal or authorization is inferred by this interface.
pub trait CatalogRepository {
    fn get(&self, digest: &str) -> Result<ValidatedCatalog>;
    fn publish(&mut self, catalog: &ValidatedCatalog) -> Result<()>;
    fn state(&self) -> Result<ActivationState>;
    fn compare_exchange(&mut self, expected_revision: u64, next: ActivationState) -> Result<()>;
}
pub fn activate(
    repo: &mut impl CatalogRepository,
    expected_revision: u64,
    catalog: &ValidatedCatalog,
) -> Result<ActivationState> {
    let next = repo
        .state()?
        .prepare_activation(expected_revision, catalog)?;
    repo.publish(catalog)?;
    repo.compare_exchange(expected_revision, next.clone())?;
    Ok(next)
}
pub fn retain(
    repo: &mut impl CatalogRepository,
    expected_revision: u64,
    reference: &str,
    digest: &str,
) -> Result<ActivationState> {
    text(reference)?;
    let catalog = repo.get(digest)?;
    ensure(
        catalog.digest() == digest,
        "artifact storage digest mismatch",
    )?;
    let mut next = repo.state()?;
    if next.revision != expected_revision {
        return Err(Error::RevisionConflict);
    }
    if let Some(old) = next.retained.get(reference) {
        ensure(old == digest, "retained reference cannot change catalog")?;
        return Ok(next);
    }
    ensure(next.retained.len() < 100_000, "retention reference bound")?;
    next.retained.insert(reference.into(), digest.into());
    next.revision = next
        .revision
        .checked_add(1)
        .ok_or_else(|| Error::Invalid("activation revision exhausted".into()))?;
    repo.compare_exchange(expected_revision, next.clone())?;
    Ok(next)
}
pub fn release(
    repo: &mut impl CatalogRepository,
    expected_revision: u64,
    reference: &str,
) -> Result<ActivationState> {
    let mut next = repo.state()?;
    if next.revision != expected_revision {
        return Err(Error::RevisionConflict);
    }
    ensure(
        next.retained.remove(reference).is_some(),
        "unknown retained reference",
    )?;
    next.revision = next
        .revision
        .checked_add(1)
        .ok_or_else(|| Error::Invalid("activation revision exhausted".into()))?;
    repo.compare_exchange(expected_revision, next.clone())?;
    Ok(next)
}
/// Deterministic test/embedded adapter. This is not fleet activation authority.
#[derive(Default)]
pub struct MemoryRepository {
    pub state: ActivationState,
    artifacts: BTreeMap<String, Vec<u8>>,
}
impl CatalogRepository for MemoryRepository {
    fn get(&self, hash: &str) -> Result<ValidatedCatalog> {
        ValidatedCatalog::from_artifact(
            self.artifacts
                .get(hash)
                .ok_or_else(|| Error::Invalid("catalog artifact not found".into()))?,
        )
    }
    fn publish(&mut self, c: &ValidatedCatalog) -> Result<()> {
        let bytes = c.canonical_bytes()?;
        if let Some(old) = self.artifacts.get(c.digest()) {
            ensure(old == &bytes, "artifact digest conflict")?;
        } else {
            self.artifacts.insert(c.digest().into(), bytes);
        }
        Ok(())
    }
    fn state(&self) -> Result<ActivationState> {
        Ok(self.state.clone())
    }
    fn compare_exchange(&mut self, expected: u64, next: ActivationState) -> Result<()> {
        if self.state.revision != expected {
            return Err(Error::RevisionConflict);
        }
        ensure(
            next.revision
                == expected
                    .checked_add(1)
                    .ok_or_else(|| Error::Invalid("revision overflow".into()))?,
            "CAS revision must advance once",
        )?;
        self.state = next;
        Ok(())
    }
}
impl MemoryRepository {
    pub fn remove(&mut self, hash: &str) -> Result<()> {
        if !self.state.can_remove(hash) {
            return Err(Error::Retained);
        }
        self.artifacts.remove(hash);
        Ok(())
    }
}
