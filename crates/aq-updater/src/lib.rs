//! Host-side release updater: config, durable journal, history and lock,
//! release verification, the runtime boundary and the update engine. The
//! Docker runtime and the CLI arrive in a later task.

pub mod config;
pub mod engine;
pub mod journal;
pub mod runtime;
pub mod verify;
