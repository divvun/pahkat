use std::path::PathBuf;

use structopt::StructOpt;
use url::Url;

use pahkat_server_core::{repo, Request};

#[derive(Debug, StructOpt)]
#[structopt()]
struct Args {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
struct RepoInitCommand {
    #[structopt(short = "u", long, parse(try_from_str = Url::parse))]
    base_url: Option<Url>,

    #[structopt(short, long)]
    name: Option<String>,

    #[structopt(short, long)]
    description: Option<String>,

    #[structopt(parse(from_os_str))]
    output_path: Option<PathBuf>,
}

impl RepoInitCommand {
    fn to_partial<'a>(&'a self) -> repo::init::PartialInitRequest<'a> {
        repo::init::PartialInitRequest::builder()
            .path(self.output_path.as_ref().map(|x| &**x))
            .base_url(self.base_url.as_ref())
            .name(self.name.as_ref().map(|x| &**x))
            .description(self.description.as_ref().map(|x| &**x))
            .build()
    }
}

#[derive(Debug, StructOpt)]
enum RepoCommand {
    Init(RepoInitCommand),
}

#[derive(Debug, StructOpt)]
enum Command {
    Repo(RepoCommand),
}

fn main() -> anyhow::Result<()> {
    let args = Args::from_args();
    println!("{:?}", args);

    match args.command {
        Command::Repo(repo) => match repo {
            RepoCommand::Init(init) => {
                let req = repo::init::InitRequest::new_from_user_input(init.to_partial())?;
                repo::init::init(req)?;
            }
        },
    }

    Ok(())
}
