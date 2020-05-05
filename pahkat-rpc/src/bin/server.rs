use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let path = if cfg!(windows) {
        format!("//./pipe/pahkat")
    } else {
        format!("/tmp/pahkat")
    };

    pahkat_rpc::start(Path::new(&path), config_path(), tokio::sync::mpsc::unbounded_channel().1).await
}

#[cfg(feature = "prefix")]
fn config_path() -> Option<&'static Path> {
    Some(Path::new("/tmp/pahkat-prefix"))
}

#[cfg(not(feature = "prefix"))]
fn config_path() -> Option<&'static Path> {
    None
}
