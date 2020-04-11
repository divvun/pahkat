#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let path = if cfg!(windows) {
        format!("//./pipe/pahkat")
    } else {
        format!("/tmp/pahkat")
    };

    pahkat_rpc::start(path, std::path::Path::new("/tmp/pahkat-prefix")).await
}
