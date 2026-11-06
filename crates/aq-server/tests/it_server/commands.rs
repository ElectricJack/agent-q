//! Spec 0006 phase 1: the command registry is the contract.
//! - `protocol/commands/v1/commands.json` is generated from the registry and
//!   must not drift (`AQ_REGENERATE_COMMANDS=1` rewrites it).
//! - Deny by default: every route aq-server mounts is a registered command
//!   route or on the reviewed list in `protocol/commands/v1/routes.json`.
use aq_command_registry::routes::{self, Class};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn commands_json_is_generated_from_the_registry() {
    let path = repo().join("protocol/commands/v1/commands.json");
    let mut generated = serde_json::to_string_pretty(&aq_server::commands::document()).unwrap();
    generated.push('\n');
    if std::env::var_os("AQ_REGENERATE_COMMANDS").is_some() {
        std::fs::write(&path, &generated).unwrap();
    }
    let published = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        published == generated,
        "protocol/commands/v1/commands.json drifted from the registry; regenerate it with \
         AQ_REGENERATE_COMMANDS=1 cargo nextest run -p aq-server --test it_server -E 'test(/^commands::/)'"
    );
}

#[test]
fn every_action_kind_is_a_command_with_a_declared_grant() {
    let registry = aq_server::commands::registry();
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../protocol/dashboard-actions/v1/schema.json"
    ))
    .unwrap();
    for kind in schema["$defs"]["ActionRequest"]["oneOf"]
        .as_array()
        .unwrap()
    {
        let kind = kind["properties"]["kind"]["const"].as_str().unwrap();
        let c = registry
            .get(kind)
            .unwrap_or_else(|| panic!("{kind} is not a command"));
        assert!(c.grant.is_some(), "{kind}");
        assert!(c.owner.is_some(), "{kind}: the owner reaches every action");
    }
    for c in registry.commands() {
        assert!(
            c.owner.is_some() || c.supervisor.is_some() || c.agent.is_some(),
            "{}",
            c.name
        );
    }
}

/// `.route(path, ...)` in the lib sources, outside `#[cfg(test)] mod` tails.
fn mounted() -> BTreeMap<String, BTreeSet<String>> {
    // Paths given by a constant rather than a literal.
    let consts: BTreeMap<(&str, &str), String> = [
        (
            ("questions.rs", "ROUTES[0].0"),
            aq_server::agent_access::questions::ROUTES[0].0.to_string(),
        ),
        (
            ("questions.rs", "ROUTES[1].0"),
            aq_server::agent_access::questions::ROUTES[1].0.to_string(),
        ),
        (
            ("routes.rs", "ROUTES[0].0"),
            aq_server::agent_access::waits::routes::ROUTES[0]
                .0
                .to_string(),
        ),
        (
            ("routes.rs", "ROUTES[1].0"),
            aq_server::agent_access::waits::routes::ROUTES[1]
                .0
                .to_string(),
        ),
        (
            ("routes.rs", "ROUTES[2].0"),
            aq_server::agent_access::waits::routes::ROUTES[2]
                .0
                .to_string(),
        ),
        (
            ("comments.rs", "add_path"),
            aq_task_comments::AGENT_ROUTES[0].0.to_string(),
        ),
        (
            ("comments.rs", "list_path"),
            aq_task_comments::AGENT_ROUTES[1].0.to_string(),
        ),
        (
            ("inbox.rs", "LIST_ROUTE"),
            aq_server::agent_access::questions::inbox::LIST_ROUTE.to_string(),
        ),
        (
            ("inbox.rs", "DETAIL_ROUTE"),
            aq_server::agent_access::questions::inbox::DETAIL_ROUTE.to_string(),
        ),
        (
            ("api.rs", "PATH"),
            "/v1/worker/profiled/material/refresh".to_string(),
        ),
    ]
    .into_iter()
    .collect();
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            // Static dashboard asset routes are generated from the bundle.
            if !name.ends_with(".rs") || name == "dashboard_assets.rs" {
                continue;
            }
            let mut text = std::fs::read_to_string(&path).unwrap();
            if let Some(i) = text.find("\n#[cfg(test)]\nmod ") {
                text.truncate(i);
            }
            let mut rest = text.as_str();
            while let Some(i) = rest.find(".route(") {
                let head = &rest[..i];
                let before = head.chars().next_back();
                rest = &rest[i + 7..];
                // A router chain (`.route(` after whitespace or a closing
                // paren) or a router variable (`routes.route(`).
                let chained = before.is_some_and(|c| c.is_whitespace() || c == ')');
                if !chained && !head.ends_with("routes") && !head.ends_with("router") {
                    continue;
                }
                let arg = rest[..rest.find(',').unwrap()].trim();
                let route = if let Some(lit) = arg.strip_prefix('"') {
                    lit.trim_end_matches('"').to_string()
                } else {
                    consts
                        .get(&(name.as_str(), arg))
                        .unwrap_or_else(|| {
                            panic!("{}: unresolved route path `{arg}`", path.display())
                        })
                        .clone()
                };
                out.entry(route).or_default().insert(name.clone());
            }
        }
    }
    out
}

#[test]
fn every_mounted_route_is_a_command_or_reviewed() {
    let registry = aq_server::commands::registry();
    let mounted = mounted();
    assert!(mounted.len() > 150, "the scan found the routes");
    let unclassified: Vec<_> = mounted
        .iter()
        .filter(|(path, _)| routes::classify(registry, path).is_none())
        .collect();
    assert!(
        unclassified.is_empty(),
        "routes outside the command registry must be reviewed and listed in \
         protocol/commands/v1/routes.json: {unclassified:?}"
    );
    for reviewed in routes::reviewed() {
        assert!(
            mounted.contains_key(&reviewed.path),
            "{} is listed but no longer mounted",
            reviewed.path
        );
        if reviewed.path.starts_with("/v1/worker/")
            || reviewed.path.starts_with("/v2/worker/")
            || reviewed.path.starts_with("/v1/execution/")
        {
            assert_eq!(reviewed.class, Class::Node, "{}", reviewed.path);
        }
    }
    // Agent-facing commands never route into the node namespaces.
    for c in registry.commands() {
        if let Some(r) = &c.agent {
            assert!(
                r.path.starts_with("/v1/agent/") || r.path.starts_with("/api/v1/commands/"),
                "{}: {}",
                c.name,
                r.path
            );
        }
    }
}
