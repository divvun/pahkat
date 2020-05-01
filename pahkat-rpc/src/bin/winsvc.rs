use pahkat_rpc::server::cli;
use std::path::Path;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = cli::Opts::from_args();

    match opt {
        #[cfg(windows)]
        cli::Opts::Service(ref opts) => {
            pahkat_rpc::server::windows::cli::run_service_command(opts).await?;
            return Ok(());
        }
    }
}
