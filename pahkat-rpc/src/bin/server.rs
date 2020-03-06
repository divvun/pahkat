#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = if cfg!(windows) {
        format!("//./pipe/pahkat")
    } else {
        format!("/tmp/pahkat")
    };

    pahkat_rpc::start(path).await
}
