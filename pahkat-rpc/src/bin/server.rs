use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = if cfg!(windows) {
        format!("//./pipe/pahkat")
    } else {
        format!("/tmp/pahkat")
    };

    match pahkat_rpc::server::setup_logger("service") {
        Ok(_) => log::debug!("Logging started."),
        Err(e) => {
            eprintln!("Error setting up logging:");
            eprintln!("{:?}", e);
            eprintln!("Attempting env_logger...");
            env_logger::try_init()?;
        }
    }


    pahkat_rpc::start(
        Path::new(&path),
        config_path(),
        tokio::sync::mpsc::unbounded_channel().1,
    )
    .await
}

#[cfg(feature = "prefix")]
fn config_path() -> Option<&'static Path> {
    Some(Path::new("/tmp/pahkat-prefix"))
}

#[cfg(not(feature = "prefix"))]
fn config_path() -> Option<&'static Path> {
    None
}
