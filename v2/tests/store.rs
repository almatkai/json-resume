use json_resume_v2::store::{transaction, Observation};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn file(dir: &Path) -> std::path::PathBuf {
    dir.join("saved-resumes/storage.json")
}
fn edit(dir: &Path, change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let path = file(dir);
    let mut value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    change(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}
fn key(url: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, url.as_bytes())
        .as_ref()
        .iter()
        .flat_map(|b| {
            let hex = b"0123456789abcdef";
            [
                hex[usize::from(b >> 4)] as char,
                hex[usize::from(b & 15)] as char,
            ]
        })
        .collect()
}

#[test]
fn absence_history_survives_reopen_and_repeated_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let never_saved = transaction(dir.path())
        .unwrap()
        .store
        .observe("private-url");
    assert_eq!(never_saved, Observation::Absent { generation: 0 });
    for _ in 0..2 {
        let old = transaction(dir.path())
            .unwrap()
            .store
            .observe("private-url");
        let mut writer = transaction(dir.path()).unwrap();
        writer
            .save("private-url", Some("New".into()), None)
            .unwrap();
        let present = writer.store.observe("private-url");
        writer.remove("private-url", Some(&present)).unwrap();
        drop(writer);
        let before = fs::read(file(dir.path())).unwrap();
        assert!(!String::from_utf8_lossy(&before).contains("private-url"));
        let mut late = transaction(dir.path()).unwrap();
        assert!(late
            .save("private-url", Some("Late".into()), Some(&old))
            .unwrap_err()
            .contains("changed"));
        assert!(late.store.records.is_empty());
        assert_eq!(fs::read(file(dir.path())).unwrap(), before);
        // A deliberate fresh Save uses a new observation and remains available.
        let fresh = late.store.observe("private-url");
        late.save("private-url", Some("Manual".into()), Some(&fresh))
            .unwrap();
        assert!(late.remove("private-url", Some(&present)).is_err());
        let current = late.store.observe("private-url");
        late.remove("private-url", Some(&current)).unwrap();
    }
}

#[test]
fn unrelated_writes_do_not_invalidate_absence_or_present_observations() {
    let dir = tempfile::tempdir().unwrap();
    let mut tx = transaction(dir.path()).unwrap();
    let absent = tx.store.observe("A");
    tx.save("B", None, None).unwrap();
    tx.save("A", Some("Ada".into()), Some(&absent)).unwrap();
    let present = tx.store.observe("A");
    tx.save("B", Some("Bee".into()), None).unwrap();
    tx.save("A", None, Some(&present)).unwrap();
    assert_eq!(
        tx.store
            .records
            .iter()
            .map(|r| r.url.as_str())
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    assert_eq!(tx.store.records[0].title.as_deref(), Some("Ada"));
    assert!(tx.save("A", None, Some(&present)).is_err());
}

#[test]
fn history_limit_keeps_evidence_and_bytes_but_allows_known_url_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let mut tx = transaction(dir.path()).unwrap();
    tx.save("known", None, None).unwrap();
    tx.remove("known", None).unwrap();
    drop(tx);
    let before = edit(dir.path(), |v| {
        for i in 1..4096 {
            v["generations"][key(&format!("synthetic-{i}"))] = json!(1);
        }
    });
    let mut tx = transaction(dir.path()).unwrap();
    let observation = tx.store.observe("known");
    assert!(tx.save("new", None, None).unwrap_err().contains("4096"));
    assert_eq!(fs::read(file(dir.path())).unwrap(), before);
    assert_eq!(tx.store.observe("known"), observation);
    tx.save("known", None, Some(&observation)).unwrap();
    let current = tx.store.observe("known");
    tx.remove("known", Some(&current)).unwrap();
    assert!(tx.save("known", None, Some(&observation)).is_err());
}

#[test]
fn invalid_history_and_unsupported_formats_preserve_bytes() {
    let dir = tempfile::tempdir().unwrap();
    transaction(dir.path())
        .unwrap()
        .save("A", None, None)
        .unwrap();
    let original = fs::read(file(dir.path())).unwrap();
    for change in [
        json!({"version":1,"revision":1,"records":[]}),
        json!({"version":9,"revision":1,"records":[]}),
        json!({"version":2,"revision":1,"records":[],"generations":{"not-a-hash":1}}),
        json!({"version":2,"revision":1,"records":[],"generations":{key("A"):0}}),
        json!({"version":2,"revision":1,"records":[],"generations":{key("A"):2}}),
    ] {
        let bytes = serde_json::to_vec(&change).unwrap();
        fs::write(file(dir.path()), &bytes).unwrap();
        let error = transaction(dir.path()).err().unwrap();
        assert!(error.contains(if change["version"] == 2 {
            "corrupt"
        } else {
            "Unsupported"
        }));
        assert_eq!(fs::read(file(dir.path())).unwrap(), bytes);
    }
    let mut legacy: Value = serde_json::from_slice(&original).unwrap();
    legacy["version"] = json!(1);
    legacy.as_object_mut().unwrap().remove("generations");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(file(dir.path()), &bytes).unwrap();
    assert!(transaction(dir.path())
        .err()
        .unwrap()
        .contains("Unsupported"));
    assert_eq!(fs::read(file(dir.path())).unwrap(), bytes);
    let duplicate = br#"{"version":2,"version":2,"revision":0,"records":[],"generations":{}}"#;
    fs::write(file(dir.path()), duplicate).unwrap();
    assert!(transaction(dir.path()).is_err());
    assert_eq!(fs::read(file(dir.path())).unwrap(), duplicate);
    fs::write(file(dir.path()), original).unwrap();
    let before = edit(dir.path(), |v| {
        v["generations"][key("A")] = json!(0);
    });
    assert!(transaction(dir.path()).is_err());
    assert_eq!(fs::read(file(dir.path())).unwrap(), before);
}

#[test]
fn overflow_lock_and_failed_commit_do_not_change_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut tx = transaction(dir.path()).unwrap();
    tx.save("A", None, None).unwrap();
    assert!(transaction(dir.path()).err().unwrap().contains("busy"));
    drop(tx);
    let before = edit(dir.path(), |v| {
        v["revision"] = json!(u64::MAX);
    });
    let mut tx = transaction(dir.path()).unwrap();
    let observation = tx.store.observe("A");
    assert!(tx.save("B", None, None).is_err());
    assert!(tx.remove("A", Some(&observation)).is_err());
    assert_eq!(tx.store.observe("A"), observation);
    assert_eq!(fs::read(file(dir.path())).unwrap(), before);
    drop(tx);
    edit(dir.path(), |v| {
        v["revision"] = json!(1);
    });
    let mut tx = transaction(dir.path()).unwrap();
    // A real serialized-size failure occurs before tempfile replacement.
    let mut rejected = false;
    for i in 0..100 {
        let bytes = fs::read(file(dir.path())).unwrap();
        let snapshot = serde_json::to_vec(&tx.store).unwrap();
        if let Err(error) = tx.save(&format!("large-{i}"), Some("x".repeat(16384)), None) {
            assert!(error.contains("1 MiB"));
            assert_eq!(fs::read(file(dir.path())).unwrap(), bytes);
            assert_eq!(serde_json::to_vec(&tx.store).unwrap(), snapshot);
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    tx.remove("A", None).unwrap();
}
