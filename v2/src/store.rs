use chrono::{SecondsFormat, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;
const MAX_STORE: u64 = 1024 * 1024;
const MAX_GENERATIONS: usize = 4096;
const STORE_VERSION: u32 = 2;

/// Identity of one saved record, independent of absence history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordIdentity {
    pub id: String,
    pub stamp: u64,
}
/// Snapshot used by asynchronous writes, including a URL that is currently absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    Present(RecordIdentity),
    Absent { generation: u64 },
}
fn url_key(url: &str) -> String {
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
fn generation_limit() -> String {
    "Saved URL history reached its limit of 4096 distinct URLs. Original storage was preserved; no history can be safely evicted.".into()
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub last_fetched: String,
    pub stamp: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    pub version: u32,
    pub revision: u64,
    pub records: Vec<Record>,
    generations: BTreeMap<String, u64>,
}
pub struct Transaction {
    dir: PathBuf,
    _lock: File,
    pub store: Store,
}
fn err() -> String {
    "Could not access saved resumes. Original storage was preserved; fix access and retry.".into()
}
fn corrupt() -> String {
    "Saved resumes are corrupt or exceed limits. Original bytes were preserved; restore a valid private backup and retry.".into()
}
pub fn transaction(data: &Path) -> Result<Transaction, String> {
    let dir = data.join("saved-resumes");
    if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
        return Err(err());
    }
    fs::create_dir_all(&dir).map_err(|_| err())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|_| err())?;
    }
    let lock_path = dir.join("transaction.lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(err());
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|_| err())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        lock.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| err())?;
    }
    lock.try_lock_exclusive()
        .map_err(|_| "Saved resumes are busy. Retry the action.".to_owned())?;
    let path = dir.join("storage.json");
    let store = match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store {
            version: STORE_VERSION,
            ..Store::default()
        },
        Err(_) => return Err(err()),
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_STORE {
                return Err(corrupt());
            }
            let mut bytes = Vec::new();
            File::open(&path)
                .map_err(|_| err())?
                .take(MAX_STORE + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| err())?;
            if bytes.len() > MAX_STORE as usize {
                return Err(corrupt());
            }
            let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            if value.get("version").and_then(serde_json::Value::as_u64)
                != Some(u64::from(STORE_VERSION))
            {
                return Err("Unsupported saved-resume storage version; expected format 2 with URL history. Original bytes were preserved.".into());
            }
            // Decode the original bytes so duplicate struct fields remain errors.
            let store: Store = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            let mut urls = std::collections::HashSet::new();
            let mut ids = std::collections::HashSet::new();
            if store.version != STORE_VERSION
                || store.generations.len() > MAX_GENERATIONS
                || store.generations.iter().any(|(key, generation)| {
                    key.len() != 64
                        || !key
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                        || *generation == 0
                        || *generation > store.revision
                })
                || store.records.len() > 256
                || store.records.iter().any(|r| {
                    r.url.is_empty()
                        || r.url.len() > 8192
                        || r.title.as_ref().is_some_and(|t| t.len() > 16384)
                        || Uuid::parse_str(&r.id).is_err()
                        || chrono::DateTime::parse_from_rfc3339(&r.last_fetched).is_err()
                        || r.stamp == 0
                        || store.generations.get(&url_key(&r.url)) != Some(&r.stamp)
                        || r.stamp > store.revision
                        || !urls.insert(&r.url)
                        || !ids.insert(&r.id)
                })
            {
                return Err(corrupt());
            }
            store
        }
    };
    Ok(Transaction {
        dir,
        _lock: lock,
        store,
    })
}
impl Store {
    pub fn identity(&self, url: &str) -> Option<RecordIdentity> {
        self.records
            .iter()
            .find(|r| r.url == url)
            .map(|r| RecordIdentity {
                id: r.id.clone(),
                stamp: r.stamp,
            })
    }
    pub fn observe(&self, url: &str) -> Observation {
        match self.identity(url) {
            Some(identity) => Observation::Present(identity),
            None => Observation::Absent {
                generation: self.generations.get(&url_key(url)).copied().unwrap_or(0),
            },
        }
    }
}
impl Transaction {
    pub fn save(
        &mut self,
        url: &str,
        title: Option<String>,
        expected: Option<&Observation>,
    ) -> Result<(), String> {
        if let Some(expected) = expected {
            if self.store.observe(url) != *expected {
                return Err(
                    "Saved resumes changed while loading. Use Save URL to update this record."
                        .into(),
                );
            }
        }
        if url.is_empty() {
            return Err("No URL to save".into());
        }
        if url.len() > 8192 || title.as_ref().is_some_and(|t| t.len() > 16384) {
            return Err("Saved URL or title exceeds its limit".into());
        }
        let key = url_key(url);
        if !self.store.generations.contains_key(&key)
            && self.store.generations.len() >= MAX_GENERATIONS
        {
            return Err(generation_limit());
        }
        let revision = self.store.revision.checked_add(1).ok_or_else(corrupt)?;
        let previous = self.store.clone();
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        if let Some(r) = self.store.records.iter_mut().find(|r| r.url == url) {
            if title.as_ref().is_some_and(|t| !t.is_empty()) {
                r.title = title;
            }
            r.last_fetched = now;
            r.stamp = revision;
        } else {
            if self.store.records.len() >= 256 {
                return Err(
                    "Saved resumes reached the limit of 256. Delete an entry and retry.".into(),
                );
            }
            self.store.records.insert(
                0,
                Record {
                    id: Uuid::new_v4().to_string(),
                    url: url.into(),
                    title: title.filter(|s| !s.is_empty()),
                    last_fetched: now,
                    stamp: revision,
                },
            );
        }
        self.store.generations.insert(key, revision);
        self.store.revision = revision;
        self.commit_or_restore(previous)
    }
    pub fn remove(&mut self, url: &str, expected: Option<&Observation>) -> Result<(), String> {
        if let Some(expected) = expected {
            if self.store.observe(url) != *expected {
                return Err(
                    "This saved resume changed. Return to the saved list and retry.".into(),
                );
            }
        }
        // Removing an already absent URL is a no-op; existing evidence is retained.
        if self.store.identity(url).is_none() {
            return Ok(());
        }
        let revision = self.store.revision.checked_add(1).ok_or_else(corrupt)?;
        let previous = self.store.clone();
        self.store.records.retain(|r| r.url != url);
        self.store.generations.insert(url_key(url), revision);
        self.store.revision = revision;
        self.commit_or_restore(previous)
    }
    fn commit_or_restore(&mut self, previous: Store) -> Result<(), String> {
        let result = self.commit();
        if result.is_err() {
            self.store = previous;
        }
        result
    }
    fn commit(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec(&self.store).map_err(|_| err())?;
        if bytes.len() > MAX_STORE as usize {
            return Err("Saved resumes exceed 1 MiB. Delete an entry and retry.".into());
        }
        let mut temp = tempfile::NamedTempFile::new_in(&self.dir).map_err(|_| err())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| err())?;
        }
        temp.write_all(&bytes).map_err(|_| err())?;
        temp.as_file().sync_all().map_err(|_| err())?;
        temp.persist(self.dir.join("storage.json"))
            .map_err(|_| err())?;
        Ok(())
    }
}
