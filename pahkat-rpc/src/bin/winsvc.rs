use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Opts {
    #[cfg(windows)]
    Service(pahkat_rpc::server::windows::cli::ServiceOpts),
}

#[cfg(windows)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opts::from_args();

    match opt {
        Opts::Service(ref opts) => {
            pahkat_rpc::server::windows::cli::run_service_command(opts).await?;
            return Ok(());
        }
    }
}

#[cfg(not(windows))]
fn main() {
    compile_error!("This can only be built on Windows.");
}
