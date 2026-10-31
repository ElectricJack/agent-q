//! Deterministic public host-contract fixture generator; no services or secrets.
fn main() {
    let value = serde_json::json!({"format_version":1,"source_revision":"a463e5918f914c8365a04dfe1b6778831cfebb95","source_archive_sha256":"48f19b80fc0734c403ef04e7181d570b9fe675597f35caf08f0b94571ca8cc70","packages":{"aq-playbooks-model":"0.1.0","aq-playbooks-runner":"0.1.1","aq-playbooks-postgres":"0.1.1","aq-playbook-host":"0.1.1"},"registry":aq_server::playbooks::domain::registry()});
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}
