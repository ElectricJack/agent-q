//! Actual profiled owner, separate from fixture execution wire and providers.
//! Runtime preparation never turns a pure proposal or qualification reference
//! into authority without the locked project/selection/source/material fences.
pub mod api;
pub mod config;
pub mod journal;
pub mod no_grant;
pub mod policies;

pub mod registration;
pub mod started;
