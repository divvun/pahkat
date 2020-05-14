use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Opts {
    #[cfg(windows)]
    Service(pahkat_rpc::server::windows::cli::ServiceOpts),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opts::from_args();

    match opt {
        #[cfg(windows)]
        Opts::Service(ref opts) => {
            pahkat_rpc::server::windows::cli::run_service_command(opts).await?;
            return Ok(());
        }
    }
}
