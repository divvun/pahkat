use std::path::PathBuf;

use structopt::StructOpt;
use url::Url;

use pahkat_repomgr::{package, repo, Request};
use pahkat_types::package::Version;

#[derive(Debug, StructOpt)]
#[structopt()]
struct Args {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
struct RepoInitCommand {
    #[structopt(short = "u", long, parse(try_from_str = Url::parse))]
    url: Option<Url>,

    #[structopt(short, long)]
    name: Option<String>,

    #[structopt(short, long)]
    description: Option<String>,

    #[structopt(parse(from_os_str))]
    output_path: Option<PathBuf>,
}

impl RepoInitCommand {
    fn to_partial<'a>(&'a self) -> repo::init::PartialRequest<'a> {
        repo::init::PartialRequest::builder()
            .path(self.output_path.as_ref().map(|x| &**x))
            .url(self.url.as_ref())
            .name(self.name.as_ref().map(|x| &**x))
            .description(self.description.as_ref().map(|x| &**x))
            .build()
    }
}

#[derive(Debug, StructOpt)]
struct RepoIndexCommand {
    #[structopt(parse(from_os_str))]
    repo_path: Option<PathBuf>,
}

impl RepoIndexCommand {
    fn to_partial<'a>(&'a self) -> repo::indexing::PartialRequest<'a> {
        repo::indexing::PartialRequest::builder()
            .path(self.repo_path.as_ref().map(|x| &**x))
            .build()
    }
}

#[derive(Debug, StructOpt)]
struct PackageInitCommand {
    id: Option<String>,

    #[structopt(short, long)]
    name: Option<String>,

    #[structopt(short, long)]
    description: Option<String>,

    #[structopt(short, long)]
    tags: Vec<String>,

    #[structopt(short = "-r", long, parse(from_os_str))]
    repo_path: Option<PathBuf>,
}

impl PackageInitCommand {
    fn to_partial<'a>(&'a self) -> package::init::PartialRequest<'a> {
        package::init::PartialRequest::builder()
            .id(self.id.as_ref().map(|x| &**x))
            .name(self.name.as_ref().map(|x| &**x))
            .description(self.description.as_ref().map(|x| &**x))
            .tags(Some(&self.tags))
            .repo_path(self.repo_path.as_ref().map(|x| &**x))
            .build()
    }
}

#[derive(Debug, StructOpt)]
struct PackageUpdateCommand {
    id: Option<String>,

    #[structopt(short = "-r", long, parse(from_os_str))]
    repo_path: Option<PathBuf>,

    #[structopt(short = "-i", long, parse(from_os_str))]
    payload_path: Option<PathBuf>,

    #[structopt(short, long)]
    platform: Option<String>,

    #[structopt(short, long)]
    channel: Option<String>,

    #[structopt(short, long)]
    version: Option<Version>,
}

impl PackageUpdateCommand {
    fn to_partial<'a>(&'a self) -> package::update::PartialRequest<'a> {
        package::update::PartialRequest::builder()
            .id(self.id.as_ref().map(|x| &**x))
            .platform(self.platform.as_ref().map(|x| &**x))
            .version(self.version.as_ref().map(|x| &*x))
            .payload_path(self.payload_path.as_ref().map(|x| &**x))
            .repo_path(self.repo_path.as_ref().map(|x| &**x))
            .channel(self.channel.as_ref().map(|x| &**x))
            .build()
    }
}

#[derive(Debug, StructOpt)]
enum RepoCommand {
    Init(RepoInitCommand),
    Index(RepoIndexCommand),
}

#[derive(Debug, StructOpt)]
enum PackageCommand {
    Init(PackageInitCommand),
    Update(PackageUpdateCommand),
}

#[derive(Debug, StructOpt)]
enum Command {
    Repo(RepoCommand),
    Package(PackageCommand),
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::from_args();
    println!("{:?}", args);

    match args.command {
        Command::Repo(repo) => match repo {
            RepoCommand::Init(init) => {
                let req = repo::init::Request::new_from_user_input(init.to_partial())?;
                repo::init::init(req)?;
            }
            RepoCommand::Index(index) => {
                let req = repo::indexing::Request::new_from_user_input(index.to_partial())?;
                repo::indexing::index(req)?;
            }
        },
        Command::Package(package) => match package {
            PackageCommand::Init(init) => {
                let req = package::init::Request::new_from_user_input(init.to_partial())?;
                package::init::init(req)?;
            }
            PackageCommand::Update(update) => {
                let req = package::update::Request::new_from_user_input(update.to_partial())?;
                package::update::update(req)?;
            }
        },
    }

    Ok(())
}
