use json_resume_v2::{adapter::strict_ack, resume};
use serde_json::{json, Value};
#[test]
fn executed_original_oracles() {
    let oracle: Value = serde_json::from_str(include_str!("../qa/oracles.json")).unwrap();
    for (i, c) in oracle["validation"].as_array().unwrap().iter().enumerate() {
        let mut v = c["input"].clone();
        if let Some(s) = v.as_str() {
            v = serde_json::from_str(s).unwrap_or(v);
        }
        assert_eq!(
            resume::validate(&v).is_ok(),
            c["valid"].as_bool().unwrap(),
            "schema fixture {i}: {}",
            c["input"]
        );
    }
    for c in oracle["markdownErrors"].as_array().unwrap() {
        assert!(resume::validate_markdown_values(c).is_err());
    }
    for c in oracle["markdown"].as_array().unwrap() {
        assert_eq!(resume::markdown(&c["input"]), c["output"].as_str().unwrap());
    }
}
#[test]
fn strict_producer_acks() {
    for v in [json!({"ok":true}), json!({"ok":true,"message":"Copied"})] {
        assert!(strict_ack(&v));
    }
    for v in [
        json!(true),
        Value::Null,
        json!({}),
        json!({"ok":false}),
        json!({"ok":true,"error":"x"}),
        json!({"ok":true,"message":null}),
        json!({"ok":"true"}),
        json!({"ok":true,"extra":1}),
    ] {
        assert!(!strict_ack(&v));
    }
}
#[test]
fn raw_markdown_and_bounds() {
    assert!(resume::validate(&json!({"work":[{"startDate":"2020-19-39"}]})).is_ok());
    assert!(resume::validate(&json!({"work":[{"startDate":"2020-20-40"}]})).is_err());
    for s in ["{}", "\"{}\"", "\"\\\"{}\\\"\""] {
        assert!(resume::parse(s.as_bytes(), true).is_ok());
    }
    assert!(resume::parse(b"not json", false).is_err());
    assert!(resume::parse(b"null", true).is_err());
    assert!(resume::parse(&vec![b' '; resume::MAX_BODY + 1], false).is_err());
    let mut v = json!({});
    for _ in 0..66 {
        v = json!({"x":v});
    }
    assert!(resume::bounded(&v).is_err());
}
#[test]
fn url_normalization() {
    let oracle: Value = serde_json::from_str(include_str!("../qa/oracles.json")).unwrap();
    for c in oracle["urlCases"].as_array().unwrap() {
        let input = c["input"].as_str().unwrap();
        if input.starts_with("ftp:") || input.contains("u:p") {
            assert!(json_resume_v2::http::source_url(input).is_err());
            continue;
        }
        let got = json_resume_v2::http::source_url(input)
            .ok()
            .map(|u| u.to_string());
        assert_eq!(got, c["normalized"].as_str().map(str::to_owned));
    }
}
#[test]
fn atomic_private_storage_conflicts_and_corruption() {
    use json_resume_v2::store::transaction;
    let dir = tempfile::tempdir().unwrap();
    let mut tx = transaction(dir.path()).unwrap();
    tx.save("A", Some("Ada".into()), None).unwrap();
    drop(tx);
    let mut tx = transaction(dir.path()).unwrap();
    let old = tx.store.observe("A");
    tx.save("B", Some("Bee".into()), None).unwrap();
    tx.save("A", None, None).unwrap();
    assert_eq!(
        tx.store
            .records
            .iter()
            .map(|r| r.url.as_str())
            .collect::<Vec<_>>(),
        vec!["B", "A"]
    );
    assert_eq!(tx.store.records[1].title.as_deref(), Some("Ada"));
    assert!(tx.remove("A", Some(&old)).is_err());
    let current = tx.store.observe("A");
    tx.remove("A", Some(&current)).unwrap();
    tx.save("A", None, None).unwrap();
    assert!(tx.remove("A", Some(&current)).is_err());
    drop(tx);
    let file = dir.path().join("saved-resumes/storage.json");
    let raw = b"{broken personal bytes";
    std::fs::write(&file, raw).unwrap();
    assert!(transaction(dir.path()).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), raw);
}
#[test]
fn complete_schema3_validation() {
    let schema: Value = serde_json::from_str(include_str!(
        "../qa/pinned/host-schemas__extension-v3.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let manifest: Value = serde_json::from_str(include_str!("../extension.json")).unwrap();
    assert!(validator.is_valid(&manifest));
    if let Ok(path) = std::env::var("JSON_RESUME_TEST_PACKAGE_MANIFEST") {
        let packaged: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(validator.is_valid(&packaged));
        assert!(packaged["files"].as_object().unwrap().len() > 400);
    }
    for (field, bad) in [
        ("timeoutMs", json!(300001)),
        ("refreshMs", json!(0)),
        ("concurrency", json!("parallel")),
        ("platforms", json!([])),
        ("runtime", json!("node")),
    ] {
        let mut v = manifest.clone();
        v["commands"][0][field] = bad;
        assert!(!validator.is_valid(&v));
    }
    let mut with_file = manifest.clone();
    with_file["files"] = json!({"resources/icon.png":"0".repeat(64)});
    assert!(validator.is_valid(&with_file));
    for path in [
        "../escape",
        "/absolute",
        "resources/../escape",
        "bin\\escape",
        "a//b",
    ] {
        let mut v = manifest.clone();
        v["files"] = json!({path:"0".repeat(64)});
        assert!(!validator.is_valid(&v));
    }
    let mut bad_artifact = manifest.clone();
    bad_artifact["artifacts"] = json!({"aarch64-apple-darwin":{"url":"http://example.test/file.zip","sha256":"0".repeat(64),"executable":"bin/json-resume-v2","executableSha256":"0".repeat(64)}});
    assert!(!validator.is_valid(&bad_artifact));
    for field in [
        "schemaVersion",
        "id",
        "slug",
        "name",
        "version",
        "runtime",
        "execution",
        "protocolVersion",
        "repository",
        "commands",
        "preferences",
        "replaces",
        "capabilities",
        "artifacts",
    ] {
        let mut v = manifest.clone();
        v.as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&v), "missing {field}");
    }
    for (key, bad) in [
        ("runtime", json!("javascript")),
        ("execution", json!("embedded")),
        ("protocolVersion", json!(2)),
        ("schemaVersion", json!(2)),
        ("capabilities", json!(["arbitrary"])),
        ("id", json!("raycast.json-resume")),
        ("replaces", json!([])),
        ("preferences", json!({})),
        ("extra", json!(true)),
    ] {
        let mut v = manifest.clone();
        v[key] = bad;
        assert!(!validator.is_valid(&v), "bad {key}");
    }
}
