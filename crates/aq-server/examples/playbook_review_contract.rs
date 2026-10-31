//! Catalog2 only; never rewrites catalog1 or installs an owner.
fn main() {
    let value = serde_json::json!({"format_version":1,"catalog_profile":2,"source_revision":"a463e5918f914c8365a04dfe1b6778831cfebb95","source_archive_sha256":"48f19b80fc0734c403ef04e7181d570b9fe675597f35caf08f0b94571ca8cc70","review_protocol_source_archive_sha256":"cc62507eb0ea9f281603b830c12e6685546a81725f311f24bbdbcbad62a7f108","registry":aq_server::playbooks::review_domain::CatalogProfile::HumanReview2.registry()});
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}
