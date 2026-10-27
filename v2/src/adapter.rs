use crate::{
    http, resume,
    store::{self, Observation, Record, RecordIdentity, Store},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tezbar_extension_sdk::{unchanged, view, BoxFuture, CoreResult, Extension, ExtensionContext};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Clone)]
enum Action {
    Submit,
    Save,
    Clear(Observation),
    Refetch,
    Edit,
    Search,
    OpenForm,
    View(Record),
    Delete(Record),
    Effect(Effect),
    RetryEffect(Effect),
    Reload,
}
#[derive(Clone)]
struct Effect {
    method: &'static str,
    url: String,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Form,
    List,
    Detail,
}
struct Model {
    surface: Surface,
    draft: String,
    input_url: String,
    current: Option<String>,
    resume: Option<Value>,
    markdown: String,
    records: Vec<Record>,
    revision: u64,
    client_revision: u64,
    busy: bool,
    dirty: bool,
    save_failed: bool,
    notice: Option<String>,
    actions: HashMap<String, Action>,
    field_id: String,
    seed_form: bool,
    job: watch::Sender<u64>,
    detail_record: Option<Record>,
    retry: Option<Effect>,
}
impl Model {
    fn new(search: bool) -> Self {
        Self {
            surface: if search { Surface::List } else { Surface::Form },
            draft: String::new(),
            input_url: String::new(),
            current: None,
            resume: None,
            markdown: String::new(),
            records: vec![],
            revision: 0,
            client_revision: 0,
            busy: false,
            dirty: true,
            save_failed: false,
            notice: None,
            actions: HashMap::new(),
            field_id: Uuid::new_v4().to_string(),
            seed_form: true,
            job: watch::channel(0).0,
            detail_record: None,
            retry: None,
        }
    }
    fn cancel(&mut self) {
        self.save_failed = false;
        self.revision += 1;
        self.job.send_replace(self.revision);
        self.busy = false;
    }
    fn url(&self) -> String {
        self.detail_record
            .as_ref()
            .map(|r| r.url.clone())
            .or_else(|| self.current.clone())
            .unwrap_or_else(|| self.input_url.clone())
    }
    fn add(
        &mut self,
        actions: &mut Vec<Value>,
        title: &str,
        kind: &str,
        action: Action,
        destructive: bool,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        self.actions.insert(id.clone(), action);
        let mut a = json!({"id":id,"title":title,"kind":kind});
        if destructive {
            a["style"] = json!("destructive");
        }
        let shortcut = match title {
            "Re-Fetch" => Some(("cmd", "r")),
            "Save URL" | "Unsave URL" => Some(("cmd", "s")),
            "Open Source URL" => Some(("cmd", "o")),
            "Delete" | "Delete from Saved" => Some(("ctrl", "x")),
            "Copy URL" if self.surface == Surface::List => Some(("cmd", "c")),
            "Open in Browser" if self.surface == Surface::List => Some(("cmd", "b")),
            _ => None,
        };
        if let Some((modifier, key)) = shortcut {
            a["shortcut"] = json!({"modifiers":[modifier],"key":key});
        }
        actions.push(a);
        id
    }
    fn render(&mut self) -> Value {
        self.actions.clear();
        self.dirty = false;
        let mut actions = vec![];
        let mut root;
        let title;
        match self.surface {
            Surface::Form => {
                title = "Open Resume".to_owned();
                self.add(
                    &mut actions,
                    "Open URL",
                    "submit-form",
                    Action::Submit,
                    false,
                );
                let url = self.url();
                let saved = self.records.iter().find(|r| r.url == url).map(|r| {
                    Observation::Present(RecordIdentity {
                        id: r.id.clone(),
                        stamp: r.stamp,
                    })
                });
                let can_save = saved.is_none() || self.save_failed;
                if let Some(saved) = saved {
                    self.add(
                        &mut actions,
                        "Unsave URL",
                        "action",
                        Action::Clear(saved),
                        false,
                    );
                }
                if can_save {
                    self.add(&mut actions, "Save URL", "action", Action::Save, false);
                }
                self.add(
                    &mut actions,
                    "View Saved Resumes",
                    "action",
                    Action::Search,
                    false,
                );
                let mut field = json!({"type":"Form.TextField","props":{"id":"url","title":"Resume JSON URL","placeholder":"https://example.com/resume.json","autoFocus":true,"actionId":self.field_id}});
                // Repeated replies omit a value: the renderer owns the live draft.
                if self.seed_form {
                    field["props"]["value"] = json!(self.draft);
                    self.seed_form = false;
                }
                root = json!({"type":"Form","props":{"navigationTitle":title},"children":[field]});
            }
            Surface::List => {
                title = "View Saved Resumes".into();
                let mut children = vec![];
                for r in self.records.clone() {
                    let ids = vec![
                        self.add(
                            &mut actions,
                            "View Resume",
                            "push",
                            Action::View(r.clone()),
                            false,
                        ),
                        self.add(
                            &mut actions,
                            "Open in Browser",
                            "open",
                            Action::Effect(Effect {
                                method: "host.shell.open",
                                url: r.url.clone(),
                            }),
                            false,
                        ),
                        self.add(
                            &mut actions,
                            "Copy URL",
                            "copy",
                            Action::Effect(Effect {
                                method: "host.clipboard.write",
                                url: r.url.clone(),
                            }),
                            false,
                        ),
                        self.add(
                            &mut actions,
                            "Delete",
                            "action",
                            Action::Delete(r.clone()),
                            true,
                        ),
                    ];
                    // Native date accessory delegates locale display to the host.
                    let mut props = json!({"id":r.id,"title":r.title.as_deref().filter(|s|!s.is_empty()).unwrap_or("Untitled Resume"),"icon":"Icon.Person","actionIds":ids,"accessories":[{"date":r.last_fetched,"icon":"Icon.Calendar"},{"icon":"Icon.Globe","tooltip":r.url}]});
                    if r.title.as_ref().is_some_and(|s| !s.is_empty()) {
                        props["subtitle"] = json!(r.url);
                    }
                    children.push(json!({"type":"List.Item","props":props}));
                }
                if children.is_empty() {
                    let id = self.add(
                        &mut actions,
                        "Open Resume",
                        "action",
                        Action::OpenForm,
                        false,
                    );
                    children.push(json!({"type":"List.EmptyView","props":{"title":"No Saved Resumes","description":"Open a resume first using the 'Open Resume' command to save it","icon":"Icon.Person","actionIds":[id]}}));
                }
                self.add(
                    &mut actions,
                    "Reload Saved Resumes",
                    "action",
                    Action::Reload,
                    false,
                );
                self.add(
                    &mut actions,
                    "Open Resume",
                    "action",
                    Action::OpenForm,
                    false,
                );
                root = json!({"type":"List","props":{"searchBarPlaceholder":"Search saved resumes...","__hasServerSearch":false},"children":children});
            }
            Surface::Detail => {
                title = self
                    .resume
                    .as_ref()
                    .and_then(|v| v["basics"]["name"].as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Resume")
                    .into();
                let url = self.url();
                if let Some(record) = self.detail_record.clone() {
                    self.add(
                        &mut actions,
                        "Open in Browser",
                        "open",
                        Action::Effect(Effect {
                            method: "host.shell.open",
                            url: url.clone(),
                        }),
                        false,
                    );
                    self.add(
                        &mut actions,
                        "Copy URL",
                        "copy",
                        Action::Effect(Effect {
                            method: "host.clipboard.write",
                            url: url.clone(),
                        }),
                        false,
                    );
                    self.add(
                        &mut actions,
                        "Delete from Saved",
                        "action",
                        Action::Delete(record),
                        true,
                    );
                    self.add(
                        &mut actions,
                        "Back to Saved Resumes",
                        "pop",
                        Action::Search,
                        false,
                    );
                } else {
                    let source_url = self
                        .current
                        .clone()
                        .or_else(|| self.records.first().map(|r| r.url.clone()))
                        .unwrap_or_default();
                    if !source_url.is_empty() {
                        self.add(
                            &mut actions,
                            "Open Source URL",
                            "open",
                            Action::Effect(Effect {
                                method: "host.shell.open",
                                url: source_url,
                            }),
                            false,
                        );
                    }
                    self.add(&mut actions, "Re-Fetch", "action", Action::Refetch, false);
                    let saved = self.records.iter().find(|r| r.url == url).map(|r| {
                        Observation::Present(RecordIdentity {
                            id: r.id.clone(),
                            stamp: r.stamp,
                        })
                    });
                    let can_save = saved.is_none() || self.save_failed;
                    if let Some(saved) = saved {
                        self.add(
                            &mut actions,
                            "Unsave URL",
                            "action",
                            Action::Clear(saved),
                            false,
                        );
                    }
                    if can_save {
                        self.add(&mut actions, "Save URL", "action", Action::Save, false);
                    }
                    self.add(&mut actions, "Edit URL", "action", Action::Edit, false);
                    self.add(
                        &mut actions,
                        "View Saved Resumes",
                        "action",
                        Action::Search,
                        false,
                    );
                }
                root = json!({"type":"Detail","props":{"markdown":if self.busy&&self.resume.is_none(){"# Loading..."}else{&self.markdown},"isLoading":self.busy,"navigationTitle":title}});
            }
        }
        if let Some(effect) = self.retry.clone() {
            let title = if effect.method == "host.shell.open" {
                "Retry Open URL"
            } else {
                "Retry Copy URL"
            };
            self.add(
                &mut actions,
                title,
                "action",
                Action::RetryEffect(effect),
                false,
            );
        }
        if self.surface == Surface::Detail {
            if let Some(back) = actions
                .iter()
                .find(|a| a["title"] == "Back to Saved Resumes" || a["title"] == "Edit URL")
            {
                root["props"]["backActionId"] = back["id"].clone();
            }
        }
        let mut result = view(&title, root, actions);
        result["refreshMs"] = Value::Null;
        if let Some(notice) = &self.notice {
            result["message"] = json!(notice);
        }
        result
    }
    fn apply_store(&mut self, store: Store) {
        self.records = store.records;
    }
    fn form_revision(&mut self, p: &Value) -> Result<bool, String> {
        let r = p["formValues"]["__formRevision"]
            .as_u64()
            .ok_or("Update Tezbar to edit this form safely, then reopen the command")?;
        if r <= self.client_revision {
            return Ok(false);
        }
        self.client_revision = r;
        Ok(true)
    }
}
pub struct Adapter {
    client: reqwest::Client,
    sessions: Mutex<HashMap<String, Weak<Mutex<Model>>>>,
}
impl Adapter {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            client: http::client()?,
            sessions: Mutex::new(HashMap::new()),
        })
    }
    fn session(&self, state: &Value) -> Result<Arc<Mutex<Model>>, String> {
        self.sessions
            .lock()
            .map_err(|_| "Session unavailable")?
            .get(state["lease"].as_str().unwrap_or(""))
            .and_then(Weak::upgrade)
            .ok_or_else(|| "Session has ended. Reopen the command.".into())
    }
    fn start(
        &self,
        ctx: &ExtensionContext,
        model: Arc<Mutex<Model>>,
        url: String,
        save_only: bool,
    ) -> Result<(), String> {
        let (revision, mut cancel, expected, search, name) = {
            let mut m = model.lock().map_err(|_| "Session unavailable")?;
            m.cancel();
            m.busy = true;
            m.notice = None;
            m.dirty = true;
            let expected = store::transaction(&ctx.paths.data_dir)?.store.observe(&url);
            if !save_only {
                m.surface = Surface::Detail;
            }
            (
                m.revision,
                m.job.subscribe(),
                expected,
                m.detail_record.is_some(),
                if save_only {
                    m.resume
                        .as_ref()
                        .and_then(|v| v["basics"]["name"].as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                } else {
                    None
                },
            )
        };
        let child = ctx.clone();
        let client = self.client.clone();
        ctx.spawn(async move {
            let work = async {
                let fetched = if save_only && name.is_some() {
                    Ok(Value::Null)
                } else {
                    http::fetch(&client, &url, save_only).await
                };
                // Title lookup failures are deliberately ignored, as in saveCurrentUrl.
                let result = if save_only {
                    Ok(fetched.unwrap_or(Value::Null))
                } else {
                    fetched
                };
                tokio::select! {biased; _ = child.cancelled() => return, _ = std::future::ready(()) => {}}
                let result = result.and_then(|v| {
                    if !save_only {resume::validate_markdown_values(&v)?;}
                    Ok(v)
                });
                let mut m = match model.lock() {
                    Ok(m) => m,
                    Err(_) => return,
                };
                if m.revision != revision {
                    return;
                }
                m.busy = false;
                match result {
                    Ok(v) => {
                        let title = name.or_else(|| {
                            v["basics"]["name"]
                                .as_str()
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                        });
                        if !save_only {
                            m.markdown = resume::markdown(&v);
                            m.resume = Some(v);
                            m.current = Some(url.clone());
                        }
                        if save_only || !search {
                            let saved =
                                store::transaction(&child.paths.data_dir).and_then(|mut tx| {
                                    tx.save(
                                        &url,
                                        title,
                                        Some(&expected),
                                    )?;
                                    Ok(tx.store)
                                });
                            match saved {
                                Ok(s) => {
                                    m.apply_store(s);
                                    m.notice = Some(
                                        if save_only {
                                            "Saved URL"
                                        } else {
                                            "Resume loaded and validated"
                                        }
                                        .into(),
                                    );
                                }
                                Err(e) => {
                                    // The saved snapshot may still contain an entry deleted
                                    // elsewhere. Always expose a deliberate fresh Save retry.
                                    m.save_failed = true;
                                    m.notice = Some(e);
                                }
                            }
                        }
                    }
                    Err(error) => {
                        m.resume = None;
                        m.current = None;
                        m.notice = Some(error.clone());
                        if search {
                            m.markdown = if error.starts_with("Could not format") {"# Error"} else if error.starts_with("Validation failed") || error == "Resume must be a JSON object" || error == "Resume is not valid JSON string" {"# Invalid Resume"} else {"# Failed to load"}.into();
                        } else {
                            m.surface = Surface::Form;
                            m.seed_form = true;
                        }
                    }
                }
                m.dirty = true;
                drop(m);
                let _ = child.invalidate();
            };
            tokio::select! {biased; _=child.cancelled()=>{},_=cancel.changed()=>{},_=work=>{}}
        })
    }
    async fn effect(
        ctx: &ExtensionContext,
        model: &Arc<Mutex<Model>>,
        effect: Effect,
    ) -> CoreResult {
        // This value was removed from the independent action table BEFORE await.
        let params = if effect.method == "host.shell.open" {
            json!({"target":effect.url})
        } else {
            json!({"text":effect.url})
        };
        let result =
            tokio::time::timeout(Duration::from_secs(8), ctx.host(effect.method, params)).await;
        let confirmed = matches!(result,Ok(Ok(ref v)) if strict_ack(v));
        let mut m = model.lock().map_err(|_| "Session unavailable")?;
        if confirmed {
            m.retry = None;
            m.notice = Some(
                if effect.method == "host.shell.open" {
                    "Opened URL"
                } else {
                    "Copied URL"
                }
                .into(),
            );
        } else {
            m.retry = Some(effect);
            m.notice = Some(
                "Could not confirm the action. Check the result, then use Retry if needed.".into(),
            );
        }
        Ok(m.render())
    }
}
pub fn strict_ack(v: &Value) -> bool {
    v.as_object().is_some_and(|o| {
        o.get("ok") == Some(&Value::Bool(true))
            && o.keys().all(|k| k == "ok" || k == "message")
            && o.get("message").is_none_or(Value::is_string)
    })
}
impl Extension for Adapter {
    fn id(&self) -> &'static str {
        "com.tezbar.native.json-resume"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn commands(&self) -> &'static [&'static str] {
        &["open-resume", "search-resumes"]
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        p: &'a Value,
        state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            if operation == "extension:dispose-session" {
                if let Ok(model) = self.session(state) {
                    model.lock().map_err(|_| "Session unavailable")?.cancel();
                }
                if let Some(id) = state["lease"].as_str() {
                    self.sessions
                        .lock()
                        .map_err(|_| "Session unavailable")?
                        .remove(id);
                }
                return Ok(unchanged());
            }
            if operation == "extension:run-command" {
                if !self.commands().contains(&command) {
                    return Err("Unknown command".into());
                }
                // A scope-owned lease releases the only enduring strong reference,
                // even when open is cancelled after this handler returns.
                let model = Arc::new(Mutex::new(Model::new(command == "search-resumes")));
                {
                    let mut m = model.lock().map_err(|_| "Session unavailable")?;
                    match store::transaction(&ctx.paths.data_dir) {
                        Ok(tx) => m.apply_store(tx.store),
                        Err(e) => m.notice = Some(e),
                    }
                }
                let key = Uuid::new_v4().to_string();
                {
                    let mut sessions = self.sessions.lock().map_err(|_| "Session unavailable")?;
                    sessions.retain(|_, v| v.strong_count() > 0);
                    if sessions.len() >= 128 {
                        return Err("Too many open resume views".into());
                    }
                    sessions.insert(key.clone(), Arc::downgrade(&model));
                }
                let lease = model.clone();
                let child = ctx.clone();
                ctx.spawn(async move {
                    child.cancelled().await;
                    drop(lease);
                })?;
                state["lease"] = json!(key);
                return Ok(model.lock().map_err(|_| "Session unavailable")?.render());
            }
            let model = self.session(state)?;
            match operation {
                "extension:refresh-session" => {
                    let mut m = model.lock().map_err(|_| "Session unavailable")?;
                    return Ok(if m.dirty { m.render() } else { unchanged() });
                }
                "extension:search-text-changed" => return Ok(unchanged()), // native list filtering
                "extension:invoke-action" => {}
                _ => return Err("Unsupported operation".into()),
            }
            let id = p["actionId"].as_str().ok_or("Missing action ID")?;
            let action = {
                let mut m = model.lock().map_err(|_| "Session unavailable")?;
                if id == m.field_id {
                    if !m.form_revision(p)? {
                        return Ok(unchanged());
                    }
                    // A newer draft owns cancellation even when its value is invalid.
                    let was_loading = m.busy;
                    m.cancel();
                    if was_loading {
                        m.surface = Surface::Form;
                        m.seed_form = false;
                    }
                    let Some(value) = p["formValues"]["value"].as_str() else {
                        m.notice = Some("URL must be text".into());
                        return Ok(m.render());
                    };
                    if value.len() > 8192 {
                        m.notice = Some("URL exceeds 8 KiB".into());
                        return Ok(m.render());
                    }
                    m.draft = value.into();
                    m.notice = None;
                    if was_loading {
                        m.surface = Surface::Form;
                        m.seed_form = false;
                        m.dirty = true;
                        let _ = ctx.invalidate();
                    }
                    return Ok(unchanged());
                }
                if !m.actions.contains_key(id) {
                    return Ok(unchanged());
                }
                if m.surface == Surface::Form {
                    if !m.form_revision(p)? {
                        return Ok(unchanged());
                    }
                    if let Some(value) = p["formValues"]["url"].as_str() {
                        if value.len() > 8192 {
                            m.cancel();
                            m.notice = Some("URL exceeds 8 KiB".into());
                            return Ok(m.render());
                        }
                        m.draft = value.into();
                    }
                }
                // Consumption lives outside SDK staged Value and survives handler drop.
                {
                    let action = m.actions.remove(id).expect("known action");
                    if let Action::Effect(effect) | Action::RetryEffect(effect) = &action {
                        m.retry = Some(effect.clone());
                    }
                    m.dirty = true;
                    action
                }
            };
            match action {
                Action::Effect(effect) | Action::RetryEffect(effect) => {
                    return Self::effect(ctx, &model, effect).await
                }
                Action::Submit | Action::Refetch | Action::Save => {
                    let (url, save_only) = {
                        let mut m = model.lock().map_err(|_| "Session unavailable")?;
                        let url = if matches!(action, Action::Submit) {
                            resume::trim(&m.draft).to_owned()
                        } else {
                            m.url()
                        };
                        if let Err(e) = http::source_url(&url) {
                            m.cancel();
                            m.notice = Some(if url.is_empty() && matches!(action, Action::Save) {
                                "No URL to save".into()
                            } else {
                                e
                            });
                            m.surface = Surface::Form;
                            m.seed_form = true;
                            return Ok(m.render());
                        }
                        if matches!(action, Action::Submit) {
                            m.draft = url.clone();
                            m.input_url = url.clone();
                        }
                        (url, matches!(action, Action::Save))
                    };
                    if let Err(e) = self.start(ctx, model.clone(), url, save_only) {
                        let mut m = model.lock().map_err(|_| "Session unavailable")?;
                        m.busy = false;
                        m.notice = Some(e);
                    }
                }
                Action::View(record) => {
                    {
                        let mut m = model.lock().map_err(|_| "Session unavailable")?;
                        m.current = Some(record.url.clone());
                        m.detail_record = Some(record.clone());
                        m.resume = None;
                        m.markdown = "# Loading...".into();
                    }
                    if let Err(e) = self.start(ctx, model.clone(), record.url, false) {
                        let mut m = model.lock().map_err(|_| "Session unavailable")?;
                        m.busy = false;
                        m.notice = Some(e);
                    }
                }
                Action::Clear(expected) => {
                    let mut m = model.lock().map_err(|_| "Session unavailable")?;
                    m.cancel();
                    let url = m.url();
                    match store::transaction(&ctx.paths.data_dir).and_then(|mut tx| {
                        tx.remove(&url, Some(&expected))?;
                        Ok(tx.store)
                    }) {
                        Ok(s) => {
                            m.apply_store(s);
                            m.notice = Some("Cleared saved URL".into());
                        }
                        Err(e) => m.notice = Some(e),
                    }
                }
                Action::Delete(record) => {
                    let mut m = model.lock().map_err(|_| "Session unavailable")?;
                    m.cancel();
                    let expected = Observation::Present(RecordIdentity {
                        id: record.id,
                        stamp: record.stamp,
                    });
                    match store::transaction(&ctx.paths.data_dir).and_then(|mut tx| {
                        tx.remove(&record.url, Some(&expected))?;
                        Ok(tx.store)
                    }) {
                        Ok(s) => {
                            m.apply_store(s);
                            m.notice = Some("Deleted saved resume".into());
                            // Action.Push detail stays open after the parent's list reload,
                            // matching the original onDelete callback. Back remains reachable.
                        }
                        Err(e) => m.notice = Some(e),
                    }
                }
                Action::Edit | Action::OpenForm => {
                    let mut m = model.lock().map_err(|_| "Session unavailable")?;
                    m.cancel();
                    m.surface = Surface::Form;
                    m.seed_form = true;
                    m.detail_record = None;
                    m.notice = None;
                }
                Action::Search | Action::Reload => {
                    let mut m = model.lock().map_err(|_| "Session unavailable")?;
                    m.cancel();
                    m.surface = Surface::List;
                    m.detail_record = None;
                    match store::transaction(&ctx.paths.data_dir) {
                        Ok(tx) => {
                            m.apply_store(tx.store);
                            m.notice = None;
                        }
                        Err(e) => m.notice = Some(e),
                    }
                }
            }
            let result = model.lock().map_err(|_| "Session unavailable")?.render();
            Ok(result)
        })
    }
}
