//! Host-side release updater: config, durable journal, history and the
//! process lock. The engine, runtime and CLI arrive in later tasks.

pub mod config;
pub mod journal;
