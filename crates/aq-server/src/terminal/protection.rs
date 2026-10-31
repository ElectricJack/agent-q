//! Terminal-specific authenticated context over the shared local encryption primitive.
pub use crate::protected_storage::{Error, KeyFile, ProtectionConfig, Sealed};
use aq_session_protocol::{TerminalIdentity, Validate};
use serde::Serialize;
use uuid::Uuid;
use zeroize::Zeroizing;
#[derive(Debug)]
pub struct Keyring(crate::protected_storage::Keyring);
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Purpose {
    Ledger,
    Outbox { operation_id: Uuid, ordinal: u32 },
}
#[derive(Clone, Debug, Serialize)]
pub struct Context {
    pub terminal: TerminalIdentity,
    pub ledger_revision: u64,
    pub purpose: Purpose,
}
impl Keyring {
    pub fn load(config: &ProtectionConfig) -> Result<Self, Error> {
        crate::protected_storage::Keyring::load(config).map(Self)
    }
    fn associated(context: &Context) -> Result<Vec<u8>, Error> {
        context
            .terminal
            .validate()
            .map_err(|_| Error::Authentication)?;
        if matches!(context.purpose,Purpose::Outbox{operation_id,..} if operation_id.is_nil()) {
            return Err(Error::Authentication);
        }
        let mut value = serde_json::to_value(("aq.terminal-relay.protected/v1", context))
            .map_err(|_| Error::Authentication)?;
        value.sort_all_objects();
        serde_json::to_vec(&value).map_err(|_| Error::Authentication)
    }
    pub fn seal(&self, context: &Context, plaintext: &[u8]) -> Result<Sealed, Error> {
        self.0.seal(&Self::associated(context)?, plaintext)
    }
    pub fn open(&self, context: &Context, sealed: &Sealed) -> Result<Zeroizing<Vec<u8>>, Error> {
        self.0.open(&Self::associated(context)?, sealed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    fn key(dir: &std::path::Path, id: &str, byte: u8) -> KeyFile {
        let file = dir.join(id);
        std::fs::write(&file, [byte; 32]).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        KeyFile {
            id: id.into(),
            file,
        }
    }
    fn context() -> Context {
        Context {
            terminal: TerminalIdentity {
                session: aq_session_protocol::SessionIdentity::Central {
                    authority_id: Uuid::from_u128(1),
                    session_id: Uuid::from_u128(2),
                    generation: 1,
                },
                terminal_id: Uuid::from_u128(3),
                epoch: 1,
            },
            ledger_revision: 4,
            purpose: Purpose::Ledger,
        }
    }
    #[test]
    fn ciphertext_is_randomized_and_bound_to_exact_row_purpose_revision_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let k = key(dir.path(), "one", 7);
        let ring = Keyring::load(&ProtectionConfig {
            active_key_id: "one".into(),
            keys: vec![k],
        })
        .unwrap();
        let context = context();
        let a = ring.seal(&context, b"private input").unwrap();
        let b = ring.seal(&context, b"private input").unwrap();
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
        assert_eq!(
            ring.open(&context, &a).unwrap().as_slice(),
            b"private input"
        );
        let mut wrong = context.clone();
        wrong.ledger_revision += 1;
        assert!(matches!(ring.open(&wrong, &a), Err(Error::Authentication)));
        wrong = context.clone();
        wrong.terminal.epoch += 1;
        assert!(ring.open(&wrong, &a).is_err());
        wrong = context.clone();
        wrong.purpose = Purpose::Outbox {
            operation_id: Uuid::from_u128(5),
            ordinal: 0,
        };
        assert!(ring.open(&wrong, &a).is_err());
        let mut corrupt = a;
        corrupt.ciphertext[0] ^= 1;
        assert!(ring.open(&context, &corrupt).is_err());
        assert!(!format!("{ring:?}{corrupt:?}").contains("private input"));
    }
    #[test]
    fn rotation_requires_retained_old_key_and_missing_keys_never_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        let one = key(dir.path(), "one", 1);
        let two = key(dir.path(), "two", 2);
        let old = Keyring::load(&ProtectionConfig {
            active_key_id: "one".into(),
            keys: vec![one.clone()],
        })
        .unwrap()
        .seal(&context(), b"secret")
        .unwrap();
        let new = Keyring::load(&ProtectionConfig {
            active_key_id: "two".into(),
            keys: vec![one, two.clone()],
        })
        .unwrap();
        assert_eq!(new.open(&context(), &old).unwrap().as_slice(), b"secret");
        assert_eq!(new.seal(&context(), b"new").unwrap().key_id, "two");
        let missing = Keyring::load(&ProtectionConfig {
            active_key_id: "two".into(),
            keys: vec![two],
        })
        .unwrap();
        assert!(matches!(
            missing.open(&context(), &old),
            Err(Error::MissingKey)
        ));
    }
    #[test]
    fn unsafe_key_files_and_duplicate_ids_refuse_without_reading_fifo() {
        let dir = tempfile::tempdir().unwrap();
        let file = key(dir.path(), "key", 1);
        let config = |k: KeyFile| ProtectionConfig {
            active_key_id: k.id.clone(),
            keys: vec![k],
        };
        let link = dir.path().join("symlink");
        symlink(&file.file, &link).unwrap();
        assert!(
            Keyring::load(&config(KeyFile {
                id: "key".into(),
                file: link
            }))
            .is_err()
        );
        let hard = dir.path().join("hard");
        std::fs::hard_link(&file.file, &hard).unwrap();
        assert!(Keyring::load(&config(file.clone())).is_err());
        std::fs::remove_file(hard).unwrap();
        std::fs::set_permissions(&file.file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Keyring::load(&config(file.clone())).is_err());
        std::fs::set_permissions(&file.file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            Keyring::load(&ProtectionConfig {
                active_key_id: "key".into(),
                keys: vec![file.clone(), file]
            })
            .is_err()
        );
        let fifo = dir.path().join("fifo");
        nix::unistd::mkfifo(
            &fifo,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        assert!(
            Keyring::load(&config(KeyFile {
                id: "fifo".into(),
                file: fifo
            }))
            .is_err()
        );
    }
}
