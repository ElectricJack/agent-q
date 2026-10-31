fn main() {
    let path =
        std::path::PathBuf::from(std::env::args().nth(1).expect("artifact output directory"));
    std::fs::create_dir_all(&path).unwrap();
    for (name, value) in [
        ("events.schema.json", aq_observation_api::events_schema()),
        ("usage.schema.json", aq_observation_api::usage_schema()),
    ] {
        std::fs::write(
            path.join(name),
            format!("{}\n", serde_json::to_string_pretty(&value).unwrap()),
        )
        .unwrap();
    }
}
