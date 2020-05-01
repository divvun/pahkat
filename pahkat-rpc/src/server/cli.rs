use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Opts {
    #[cfg(windows)]
    Service(crate::server::windows::cli::ServiceOpts),
}
