mod support;
fn main() {
    let value = match std::env::args().nth(1).as_deref() {
        Some("fixtures") => support::fixtures(),
        Some("subject") => aq_review_protocol::subject_schema(),
        Some("decision") => aq_review_protocol::decision_schema(),
        Some("check") => aq_review_protocol::check_schema(),
        _ => panic!("use fixtures|subject|decision|check"),
    };
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}
