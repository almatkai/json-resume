use serde_json::Value;
use std::sync::OnceLock;

pub const MAX_BODY: usize = 1024 * 1024;
pub fn bounded(value: &Value) -> Result<(), String> {
    let mut count = 0;
    let mut stack = vec![(value, 0)];
    while let Some((v, depth)) = stack.pop() {
        count += 1;
        if depth > 64 || count > 20_000 {
            return Err("Resume exceeds 64 levels or 20,000 values".into());
        }
        match v {
            Value::Array(a) => stack.extend(a.iter().map(|v| (v, depth + 1))),
            Value::Object(o) => stack.extend(o.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    Ok(())
}
fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value = serde_json::from_str(include_str!("../resources/resume-schema.json"))
            .expect("embedded schema");
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft7)
            .should_validate_formats(true)
            .with_format("uri", uri_format)
            .with_format("email", email_format)
            .build(&schema)
            .expect("local schema compiles")
    })
}
// jsonschema 1.5.0's two formats used by this exact schema. URI intentionally
// allows arbitrary schemes, Unicode and malformed percent escapes, as its oracle does.
fn uri_format(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    scheme
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b))
        && !rest.chars().any(js_space)
}
pub fn js_space(c: char) -> bool {
    c.is_whitespace() && c != '\u{85}' || c == '\u{feff}'
}
pub fn trim(s: &str) -> &str {
    s.trim_matches(js_space)
}
fn email_format(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if local.split('.').any(|p| {
        p.is_empty()
            || !p
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_!#$%&'*+-/=?^`{|}~".contains(&b))
    }) {
        return false;
    }
    if domain.starts_with('[') && domain.ends_with(']') {
        let parts: Vec<_> = domain[1..domain.len() - 1].split('.').collect();
        return parts.len() == 4
            && parts.iter().all(|p| {
                !p.is_empty()
                    && p.len() <= 3
                    && p.bytes().all(|b| b.is_ascii_digit())
                    && p.parse::<u16>().is_ok_and(|n| n <= 255)
            });
    }
    let parts: Vec<_> = domain.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 63
                && p.as_bytes()[0].is_ascii_alphanumeric()
                && p.as_bytes()[p.len() - 1].is_ascii_alphanumeric()
                && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
pub fn parse(bytes: &[u8], json_mime: bool) -> Result<Value, String> {
    if bytes.len() > MAX_BODY {
        return Err("Resume exceeds 1 MiB".into());
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut value: Value = serde_json::from_str(text).map_err(|_| {
        if json_mime {
            "Response is not valid JSON"
        } else {
            "Resume is not valid JSON string"
        }
        .to_owned()
    })?;
    // The component and validateResume helper each attempt string coercion.
    let attempts = if json_mime { 2 } else { 1 };
    for _ in 0..attempts {
        if let Some(s) = value.as_str() {
            value = serde_json::from_str(s)
                .map_err(|_| "Resume is not valid JSON string".to_owned())?;
        }
    }
    validate(&value)?;
    Ok(value)
}
pub fn validate(v: &Value) -> Result<(), String> {
    bounded(v)?;
    if !v.is_object() {
        return Err("Resume must be a JSON object".into());
    }
    if validator().is_valid(v) {
        Ok(())
    } else {
        // Do not include schema error values: they can contain personal data.
        Err("Validation failed: resume does not match the JSON Resume schema".into())
    }
}
pub fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
pub fn js(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Number(n) => ryu_js::Buffer::new()
            .format(n.as_f64().unwrap_or(0.0))
            .to_owned(),
        Value::Object(_) => "[object Object]".into(),
        Value::Array(a) => a
            .iter()
            .map(|v| if v.is_null() { String::new() } else { js(v) })
            .collect::<Vec<_>>()
            .join(","),
        _ => v.to_string(),
    }
}
fn get(v: &Value, k: &str) -> String {
    v.get(k).map(js).unwrap_or_else(|| "undefined".into())
}
fn fallback(v: &Value, k: &str, d: &str) -> String {
    if truth(&v[k]) {
        get(v, k)
    } else {
        d.into()
    }
}
fn array<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v[k].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn markdown(v: &Value) -> String {
    let b = &v["basics"];
    let address = if truth(&b["location"]["address"]) {
        format!("- {}", get(&b["location"], "address"))
    } else {
        String::new()
    };
    let mut lines = vec![format!(
        "# {} {}",
        fallback(b, "name", "(no name)"),
        address
    )];
    if truth(&b["label"]) {
        lines.push(format!("**{}**", get(b, "label")));
    }
    let mut contact = vec![];
    for (k, label) in [
        ("email", "Email"),
        ("phone", "Phone"),
        ("website", "Website"),
    ] {
        if truth(&b[k]) {
            contact.push(format!("{label}: {}", get(b, k)));
        }
    }
    if !contact.is_empty() {
        lines.push(contact.join(" • "));
    }
    if truth(&b["summary"]) || !contact.is_empty() || truth(&b["label"]) {
        lines.push("---".into());
    }
    if truth(&b["summary"]) {
        lines.push(get(b, "summary"));
    }
    let profiles = array(b, "profiles");
    if !profiles.is_empty() {
        lines.extend(["---".into(), "**Profiles**".into()]);
        for p in profiles {
            let name = if truth(&p["network"]) {
                get(p, "network")
            } else if truth(&p["username"]) {
                get(p, "username")
            } else {
                get(p, "url")
            };
            lines.push(format!("- {name}: {}", get(p, "url")));
        }
    }
    let skills = array(v, "skills");
    if !skills.is_empty() {
        lines.extend(["---".into(), "## Skills".into()]);
        for s in skills {
            lines.push(format!(
                "- **{}** — {}",
                fallback(s, "name", "Skill"),
                array(s, "keywords")
                    .iter()
                    .take(8)
                    .map(js)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    let work = array(v, "work");
    if !work.is_empty() {
        lines.extend(["---".into(), "## Work Experience".into()]);
        for w in work {
            lines.push(format!(
                "\n### {} — {}",
                fallback(w, "position", "(position)"),
                fallback(w, "company", "(company)")
            ));
            let dates = dates(w);
            if !dates.is_empty() {
                lines.push(format!("*{dates}*"));
            }
            if truth(&w["summary"]) {
                lines.push(format!("\n{}", get(w, "summary")));
            }
            let h = array(w, "highlights");
            if !h.is_empty() {
                lines.push("\n**Highlights**".into());
                for h in h {
                    lines.push(format!("- {}", js(h)));
                }
            }
        }
    }
    let education = array(v, "education");
    if !education.is_empty() {
        lines.extend(["---".into(), "## Education".into()]);
        for e in education {
            lines.push(format!(
                "\n**{}** — {}",
                fallback(e, "institution", "(institution)"),
                fallback(e, "area", "")
            ));
            let dates = dates(e);
            if !dates.is_empty() {
                lines.push(format!("*{dates}*"));
            }
            if truth(&e["gpa"]) {
                lines.push(format!("- GPA: {}", get(e, "gpa")));
            }
        }
    }
    let languages = array(v, "languages");
    if !languages.is_empty() {
        lines.extend(["---".into(), "## Languages".into()]);
        for l in languages {
            lines.push(format!(
                "- {} — {}",
                get(l, "language"),
                fallback(l, "fluency", "")
            ));
        }
    }
    lines.join("\n\n")
}
fn dates(v: &Value) -> String {
    let end = if truth(&v["endDate"]) {
        format!(" — {}", get(v, "endDate"))
    } else {
        String::new()
    };
    trim(&format!("{}{end}", fallback(v, "startDate", ""))).into()
}

/// JSON can shadow Object.prototype.toString in the source's extra fields.
/// The original template literal throws; do not silently invent printable text.
pub fn validate_markdown_values(v: &Value) -> Result<(), String> {
    fn printable(v: &Value) -> bool {
        match v {
            Value::Object(o) => !o.contains_key("toString"),
            Value::Array(a) => a.iter().all(printable),
            _ => true,
        }
    }
    let website = &v["basics"]["website"];
    if !printable(website)
        || array(v, "work").iter().any(|w| !printable(&w["company"]))
        || array(v, "education").iter().any(|e| !printable(&e["gpa"]))
    {
        Err("Could not format this resume: a value cannot be converted to text".into())
    } else {
        Ok(())
    }
}
