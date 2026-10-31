//! Catalog2 only; never rewrites catalog1 or installs an owner.
fn main() {
    let value = serde_json::json!({"format_version":1,"catalog_profile":2,"packages":{"aq-playbook-host":"0.1.2","aq-playbooks-runner":"0.1.2","aq-playbooks-postgres":"0.1.2","aq-playbooks-model":"0.1.0","aq-playbooks-compiler":"0.1.0","aq-review-protocol":"0.1.0"},"source_revision":"d3bd349d9293b7aee6ed434cd1927ecfb035f8e7","source_archive_sha256":"bd1246e8e972ee1627e94598945f398812c89044106b463151a696511412ca39","review_protocol_source_archive_sha256":"cc62507eb0ea9f281603b830c12e6685546a81725f311f24bbdbcbad62a7f108","registry":aq_server::playbooks::review_domain::CatalogProfile::HumanReview2.registry()});
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}
