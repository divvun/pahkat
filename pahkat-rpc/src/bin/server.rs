#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let path = if cfg!(windows) {
        format!("//./pipe/pahkat")
    } else {
        format!("/tmp/pahkat")
    };

    pahkat_rpc::start(path, config_path()).await
}

#[cfg(feature = "prefix")]
fn config_path() -> Option<&'static std::path::Path> {
    Some(std::path::Path::new("/tmp/pahkat-prefix"))
}

#[cfg(not(feature = "prefix"))]
fn config_path() -> Option<&'static std::path::Path> {
    None
}
