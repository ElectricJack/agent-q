use super::protection::{Keyring, ProtectionConfig};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalConfig {
    pub protection: ProtectionConfig,
    #[serde(default)]
    pub epoch_input: bool,
}
pub struct Loaded {
    pub keyring: Keyring,
    pub epoch_input: bool,
    pub runtime_id: uuid::Uuid,
    pub runtime: super::runtime::Runtime,
}
impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalConfig")
            .field("epoch_input", &self.epoch_input)
            .finish_non_exhaustive()
    }
}
pub fn deserialize<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Arc<Loaded>>, D::Error> {
    Option::<TerminalConfig>::deserialize(d)?
        .map(|c| {
            Ok(Arc::new(Loaded {
                keyring: Keyring::load(&c.protection).map_err(serde::de::Error::custom)?,
                epoch_input: c.epoch_input,
                runtime_id: uuid::Uuid::new_v4(),
                runtime: Default::default(),
            }))
        })
        .transpose()
}
