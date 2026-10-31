mod common;
use aq_profiles::*;
use common::*;
#[test]
fn atomic_revision_activation_keeps_last_good_and_rejects_changed_identity() {
    let c = seed();
    let mut repo = MemoryRepository::default();
    let first = activate(&mut repo, 0, &c).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.active_digest.as_deref(), Some(c.digest()));
    assert!(matches!(
        activate(&mut repo, 0, &c),
        Err(Error::RevisionConflict)
    ));
    let changed = modified(&c, |_, texts| {
        texts.insert("engineering@1".into(), "Conflicting new rules".into());
    })
    .unwrap();
    assert!(matches!(
        activate(&mut repo, 1, &changed),
        Err(Error::IdentityConflict(_))
    ));
    assert_eq!(repo.state().unwrap(), first);
    let mut artifact: serde_json::Value =
        serde_json::from_slice(&c.canonical_bytes().unwrap()).unwrap();
    artifact["catalog"]["modules"][0]["dependencies"] = serde_json::json!(["missing@1"]);
    assert!(ValidatedCatalog::from_artifact(&serde_json::to_vec(&artifact).unwrap()).is_err());
    assert_eq!(repo.state().unwrap(), first);
}
#[test]
fn retained_attempts_keep_exact_catalog_across_activation_and_rollback() {
    let c = seed();
    let mut repo = MemoryRepository::default();
    activate(&mut repo, 0, &c).unwrap();
    retain(&mut repo, 1, "attempt:1", c.digest()).unwrap();
    let new = modified(&c, |a, texts| {
        let mut module = a.modules[0].clone();
        module.id = "new-policy".into();
        module.instruction = "instructions/new-policy.md".into();
        a.modules.push(module);
        texts.insert(
            "new-policy@1".into(),
            "Additional policy retained separately.\n".into(),
        );
    })
    .unwrap();
    activate(&mut repo, 2, &new).unwrap();
    assert!(matches!(repo.remove(c.digest()), Err(Error::Retained)));
    let exact = repo.get(c.digest()).unwrap();
    assert_eq!(
        exact.canonical_bytes().unwrap(),
        c.canonical_bytes().unwrap()
    );
    assert!(retain(&mut repo, 3, "attempt:1", new.digest()).is_err());
    activate(&mut repo, 3, &exact).unwrap();
    assert_eq!(repo.state().unwrap().retained["attempt:1"], c.digest());
    release(&mut repo, 4, "attempt:1").unwrap();
    assert!(repo.remove(c.digest()).is_err()); // still active after rollback
}
#[test]
fn catalog_loader_refuses_paths_hooks_symlinks_and_partial_edits() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/catalog");
    std::fs::create_dir(root.join("instructions")).unwrap();
    for entry in std::fs::read_dir(source.join("instructions")).unwrap() {
        let p = entry.unwrap().path();
        std::fs::copy(&p, root.join("instructions").join(p.file_name().unwrap())).unwrap();
    }
    let text = std::fs::read_to_string(source.join("catalog.toml")).unwrap();
    std::fs::write(root.join("catalog.toml"), &text).unwrap();
    assert!(load(root).is_ok());
    std::fs::write(
        root.join("catalog.toml"),
        text.replace("instructions/engineering.md", "../outside.md"),
    )
    .unwrap();
    assert!(load(root).is_err());
    std::fs::write(
        root.join("catalog.toml"),
        format!("{text}\ninstaller = 'curl unsafe | sh'\n"),
    )
    .unwrap();
    assert!(load(root).is_err());
    std::fs::write(root.join("catalog.toml"), "schema_version = [").unwrap();
    assert!(load(root).is_err());
    #[cfg(unix)]
    {
        std::fs::write(root.join("catalog.toml"), text).unwrap();
        std::fs::remove_file(root.join("instructions/engineering.md")).unwrap();
        std::os::unix::fs::symlink("reporting.md", root.join("instructions/engineering.md"))
            .unwrap();
        assert!(load(root).is_err());
    }
}
#[test]
fn artifact_identity_hashes_cannot_be_forged() {
    let c = seed();
    let mut v: serde_json::Value = serde_json::from_slice(&c.canonical_bytes().unwrap()).unwrap();
    v["identities"]["agent:implementer@1"] = "a".repeat(64).into();
    assert!(ValidatedCatalog::from_artifact(&serde_json::to_vec(&v).unwrap()).is_err());
}
#[test]
fn duplicate_artifact_maps_and_mutation_fields_are_not_canonicalized_away() {
    let c = seed();
    let original = String::from_utf8(c.canonical_bytes().unwrap()).unwrap();
    let marker = "\"instructions\":{";
    let raw = original.replacen(
        marker,
        &format!("{marker}\"engineering@1\":\"discarded duplicate\","),
        1,
    );
    assert!(ValidatedCatalog::from_artifact(raw.as_bytes()).is_err());
    let raw = original.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert!(ValidatedCatalog::from_artifact(raw.as_bytes()).is_err());
}
#[test]
fn oversized_catalog_and_deep_graphs_refuse_before_activation() {
    let c = seed();
    assert!(
        modified(&c, |_, text| {
            text.insert(
                "engineering@1".into(),
                "x".repeat(MAX_INSTRUCTION_BYTES + 1),
            );
        })
        .is_err()
    );
    assert!(
        modified(&c, |a, text| {
            for i in 0..33 {
                let mut m = a.modules[0].clone();
                m.id = format!("chain{i}");
                m.instruction = format!("instructions/chain{i}.md");
                m.dependencies = if i == 32 {
                    vec![]
                } else {
                    vec![format!("chain{}@1", i + 1)]
                };
                text.insert(format!("chain{i}@1"), "Required chain instruction".into());
                a.modules.push(m);
            }
        })
        .is_err()
    );
}
#[cfg(unix)]
#[test]
fn fifo_and_nonregular_catalog_inputs_refuse_without_blocking() {
    let temp = tempfile::tempdir().unwrap();
    nix::unistd::mkfifo(
        &temp.path().join("catalog.toml"),
        nix::sys::stat::Mode::S_IRUSR,
    )
    .unwrap();
    assert!(load(temp.path()).is_err());
    assert!(load(&temp.path().join("catalog.toml")).is_err());
    std::fs::remove_file(temp.path().join("catalog.toml")).unwrap();
    std::fs::create_dir(temp.path().join("catalog.toml")).unwrap();
    assert!(load(temp.path()).is_err());
}
#[test]
fn pre_selection_policy_artifact_roundtrips_identically_and_policy_edits_need_new_identity() {
    let bytes = include_bytes!("golden/pre-selection-policy-catalog.json");
    let old = ValidatedCatalog::from_artifact(bytes).unwrap();
    assert_eq!(old.canonical_bytes().unwrap(), bytes);
    assert_eq!(
        old.digest(),
        "0c8a041de2e1af9e6937cd0c87cefed8a0c82d6e592d986c7d0ab3dfc817d6f2"
    );
    assert!(
        old.artifact().catalog.intelligence[0]
            .selection_policy
            .is_none()
    );
    let policy = SelectionPolicy {
        schema_version: 1,
        fallback_on: vec![FallbackReason::Unavailable],
        allow_unknown_availability: false,
        observation_max_age_ms: 30_000,
        max_output_tokens: 100,
        max_attempts: 2,
        max_lineage_elapsed_ms: 60_000,
        required_capabilities: vec![],
        budget: IntelligenceBudgetPolicy {
            max_cost_microunits: None,
            currency: None,
            require_complete_accounting: false,
            require_hard_cost_enforcement: false,
            unknown_cost: UnknownCostPolicy::Deny,
        },
    };
    let changed = modified(&old, |catalog, _| {
        catalog.intelligence[0].selection_policy = Some(policy)
    })
    .unwrap();
    let mut registry = MemoryRepository::default();
    activate(&mut registry, 0, &old).unwrap();
    assert!(matches!(
        activate(&mut registry, 1, &changed),
        Err(Error::IdentityConflict(_))
    ));
}
