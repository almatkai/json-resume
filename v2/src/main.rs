#[tokio::main(worker_threads = 2)]
async fn main() {
    let result = match json_resume_v2::adapter::Adapter::new() {
        Ok(adapter) => tezbar_extension_sdk::serve(adapter).await,
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        // SDK protocol diagnostics cannot contain resume bodies or URLs.
        let _ = error;
        eprintln!("JSON Resume stopped because its connection or request was invalid.");
        std::process::exit(1);
    }
}
