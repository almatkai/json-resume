use json_resume_v2::http;
#[tokio::test]
async fn owned_tls_fixture() {
    let Ok(url) = std::env::var("JSON_RESUME_TEST_TLS_URL") else {
        return;
    };
    let cert = std::fs::read(std::env::var("JSON_RESUME_TEST_TLS_CERT").unwrap()).unwrap();
    assert!(url.starts_with("https://127.0.0.1:"));
    let strict = http::client().unwrap();
    assert!(http::fetch(&strict, &url, false).await.is_err());
    let client = reqwest::Client::builder()
        .no_proxy()
        .add_root_certificate(reqwest::Certificate::from_pem(&cert).unwrap())
        .build()
        .unwrap();
    let v = http::fetch(&client, &url, false).await.unwrap();
    assert_eq!(v["basics"]["name"], "Synthetic TLS");
}
