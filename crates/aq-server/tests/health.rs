#[test]
fn version_info_reports_protocol_range() {
    let v = aq_server::version_info();
    assert_eq!((v.protocol_min, v.protocol_max), (1, 1));
    assert_eq!(v.server_version, env!("CARGO_PKG_VERSION"));
}
