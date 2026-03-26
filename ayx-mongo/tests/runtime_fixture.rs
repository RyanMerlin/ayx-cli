use std::fs;

#[test]
fn runtime_settings_fixture_contains_embedded_mongo_root() {
    let path = "C:/code/RuntimeSettings.xml";
    if !std::path::Path::new(path).exists() {
        eprintln!("skipping fixture test; '{}' not present", path);
        return;
    }
    let xml = fs::read_to_string(path).expect("fixture file should exist");
    assert!(
        xml.contains("EmbeddedMongoDBRootPath"),
        "expected EmbeddedMongoDBRootPath in runtime settings fixture"
    );
}
