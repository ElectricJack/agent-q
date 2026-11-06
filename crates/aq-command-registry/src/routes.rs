//! Deny by default (spec 0006 §6.5): every route aq-server mounts is either
//! the route of a registered command or on the reviewed list in
//! `protocol/commands/v1/routes.json`. aq-server's route test enumerates the
//! mounted routes and fails on anything unclassified.
use super::Registry;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    /// Served through the command registry (a command's route).
    Command,
    /// Node and worker protocol, authenticated by the node credential.
    Node,
    /// The F1b supervisor's own conversation routes.
    Supervisor,
    /// agent-access-1 routes kept while spec 0006 §9 retires them.
    AgentLegacy,
    /// Owner administration and REST mutations outside the command surface.
    OwnerAdmin,
    /// Health, readiness and static dashboard assets.
    Infra,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reviewed {
    pub path: String,
    pub class: Class,
    pub note: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    classes: serde_json::Map<String, serde_json::Value>,
    routes: Vec<Reviewed>,
}
pub const ROUTES_JSON: &str = include_str!("../../../protocol/commands/v1/routes.json");
/// The reviewed list. Panics only if the checked-in file is malformed,
/// which the crate's tests catch.
pub fn reviewed() -> Vec<Reviewed> {
    let file: File = serde_json::from_str(ROUTES_JSON).expect("routes.json");
    assert_eq!(file.version, 1);
    file.routes
}
/// How a mounted path is classified, if at all.
pub fn classify(registry: &Registry, path: &str) -> Option<Class> {
    let command = registry.commands().iter().any(|c| {
        [&c.owner, &c.supervisor, &c.agent]
            .into_iter()
            .flatten()
            .any(|r| r.path == path && r.body != super::Body::Local)
    });
    if command {
        return Some(Class::Command);
    }
    reviewed()
        .into_iter()
        .find(|r| r.path == path)
        .map(|r| r.class)
}
