//! Delegated credential primitives. These do not establish node or grant currency:
//! effect owners must load current authority under their transaction locks.
mod model;
pub use model::{CredentialKey, PermitRecord};
pub mod storage;
