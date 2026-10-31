#[path = "../tests/common/mod.rs"]
mod common;
fn main() {
    let c = common::seed();
    let r = common::request(&c);
    println!(
        "{}",
        serde_json::to_string_pretty(&aq_profiles::compile(&c, &r, &common::access()).unwrap())
            .unwrap()
    );
}
