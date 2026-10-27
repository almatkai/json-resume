use crate::resume;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use url::Url;

pub fn source_url(s: &str) -> Result<Url, String> {
    let s = resume::trim(s);
    if s.is_empty() {
        return Err("URL is required".into());
    }
    if s.len() > 8192 {
        return Err("URL exceeds 8 KiB".into());
    }
    let u = Url::parse(s).map_err(|_| "Please enter a valid URL".to_owned())?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err("Please enter an HTTP or HTTPS URL".into());
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("URLs containing credentials are not supported".into());
    }
    Ok(u)
}
pub fn client() -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .pool_max_idle_per_host(0)
        .pool_idle_timeout(Duration::ZERO)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 {
                attempt.error("Resume exceeded 5 redirects")
            } else if !matches!(attempt.url().scheme(), "http" | "https")
                || !attempt.url().username().is_empty()
                || attempt.url().password().is_some()
            {
                attempt.error("Unsupported redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Could not create a secure HTTP client".into())
}
fn mime_json(s: &str) -> bool {
    let s = s
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let Some((ty, sub)) = s.split_once('/') else {
        return false;
    };
    let valid = |v: &str, sub: bool| {
        !v.is_empty()
            && v.len() <= 127
            && v.as_bytes()[0].is_ascii_alphanumeric()
            && v.bytes().all(|b| {
                b.is_ascii_alphanumeric() || b"!#$&^_-".contains(&b) || (sub && b".+".contains(&b))
            })
    };
    if !valid(ty, false) || !valid(sub, true) {
        return false;
    }
    sub.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|word| word == "json")
}
pub async fn fetch(client: &Client, source: &str, title_only: bool) -> Result<Value, String> {
    let url = source_url(source)?;
    let work = async {
        let mut response = client.get(url).send().await.map_err(|e| {
            if e.is_timeout() {
                "Fetching the resume timed out"
            } else if e.is_redirect() {
                "Resume redirect failed (maximum 5 redirects)"
            } else {
                "Could not fetch the resume (connection or TLS error)"
            }
            .to_owned()
        })?;
        if !response.status().is_success() {
            return Err(format!(
                "Failed to fetch resume: HTTP {}",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|n| n > resume::MAX_BODY as u64)
        {
            return Err("Resume exceeds 1 MiB".into());
        }
        let json_mime = mime_json(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Could not read the resume body".to_owned())?
        {
            if bytes.len() + chunk.len() > resume::MAX_BODY {
                return Err("Resume exceeds 1 MiB".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if title_only {
            let v: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "Response is not valid JSON".to_owned())?;
            resume::bounded(&v)?;
            Ok(v)
        } else {
            resume::parse(&bytes, json_mime)
        }
    };
    tokio::time::timeout(Duration::from_secs(15), work)
        .await
        .map_err(|_| "Fetching the resume timed out".to_owned())?
}
