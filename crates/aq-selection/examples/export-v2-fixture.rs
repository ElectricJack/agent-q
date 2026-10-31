#[path = "../tests/common/mod.rs"]
mod common;
fn main() {
    let (catalog, inputs) = common::fixture();
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"catalog_artifact_json":String::from_utf8(catalog.canonical_bytes().unwrap()).unwrap(),"inputs":inputs})).unwrap());
}
