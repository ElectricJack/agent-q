use aq_observation_api::*;
use wire::{Digest, Validate};
fn examples() -> serde_json::Value {
    serde_json::from_str(include_str!("../artifacts/examples.json")).unwrap()
}
fn page() -> EventsPage {
    decode_events(&serde_json::to_vec(&examples()["events"]).unwrap()).unwrap()
}
fn usage() -> SessionUsage {
    decode_usage(&serde_json::to_vec(&examples()["usage"]).unwrap()).unwrap()
}
#[test]
fn published_examples_and_schemas_are_stable() {
    assert_eq!(serde_json::to_value(page()).unwrap(), examples()["events"]);
    assert_eq!(serde_json::to_value(usage()).unwrap(), examples()["usage"]);
    assert_eq!(
        events_schema(),
        serde_json::from_str::<serde_json::Value>(include_str!("../artifacts/events.schema.json"))
            .unwrap()
    );
    assert_eq!(
        usage_schema(),
        serde_json::from_str::<serde_json::Value>(include_str!("../artifacts/usage.schema.json"))
            .unwrap()
    );
}
#[test]
fn exact_scope_and_continuation_are_not_caller_selected() {
    let mut p = page();
    p.binding.project_id = "00000000-0000-0000-0000-000000000099".parse().unwrap();
    assert!(p.validate().is_err());
    let mut p = page();
    p.complete_for_snapshot = false;
    assert!(p.validate().is_err());
    p.next_after = Some(p.events[0].record.body.sequence);
    p.validate().unwrap();
    p.next_after = Some(p.through_sequence + 1);
    assert!(p.validate().is_err());
    let mut p = page();
    p.events.push(p.events[0].clone());
    assert!(p.validate().is_err());
    let mut p = page();
    p.after_sequence = p.through_sequence;
    assert!(p.validate().is_err());
}
#[test]
fn raw_usage_claims_do_not_become_complete_projection_or_contribution() {
    let mut p = page();
    p.events[0].accounting = AccountingMark::ObservationOnly;
    assert!(p.validate().is_err());
    let r = &p.events[0].record;
    p.events[0].accounting = AccountingMark::ContributionRecorded {
        event: wire::EventRef {
            event_id: r.event_id,
            digest: Digest::of(b"wrong"),
        },
        supersedes: None,
    };
    assert!(p.validate().is_err());
    let mut u = usage();
    u.usage.counters.input = wire::Counter::Known { value: 0 };
    assert!(u.validate().is_err());
    let mut u = usage();
    u.known_category_subtotals[0] = 1;
    assert!(u.validate().is_err());
    let mut u = usage();
    u.incomplete_calls = 1;
    assert!(u.validate().is_err());
}
#[test]
fn unknown_duplicate_fields_versions_and_oversize_are_refused() {
    let mut p = examples()["events"].clone();
    p["caller_principal"] = serde_json::json!("owner");
    assert!(decode_events(&serde_json::to_vec(&p).unwrap()).is_err());
    let raw = serde_json::to_string(&page()).unwrap().replace(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    assert!(decode_events(raw.as_bytes()).is_err());
    assert!(decode_events(&vec![b' '; MAX_PAGE_BYTES + 1]).is_err());
    assert!(decode_usage(&vec![b' '; 16385]).is_err());
    let mut p = page();
    p.schema_version = 2;
    assert!(p.validate().is_err());
}
