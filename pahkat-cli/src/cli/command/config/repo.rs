use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct RepoArgs {
    #[structopt(
        short,
        long,
        help = "Path to configuration directory [default: TODO]",
        parse(from_os_str)
    )]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Add or modify a repository entry")]
pub struct Add {
    #[structopt(help = "Repository URL")]
    pub repo_url: pahkat_types::repo::RepoUrl,

    #[structopt(help = "Repository package channel")]
    pub channel: Option<String>,

    #[structopt(flatten)]
    args: RepoArgs,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Remove a repository entry")]
pub struct Remove {
    #[structopt(help = "Repository URL")]
    pub repo_url: pahkat_types::repo::RepoUrl,

    #[structopt(flatten)]
    args: RepoArgs,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "List all repository entries")]
pub struct List {
    #[structopt(
        short,
        long,
        help = "Path to configuration directory [default: TODO]",
        parse(from_os_str)
    )]
    pub config_path: Option<PathBuf>,
}

impl crate::ConfigPath for Add {
    #[inline]
    fn config_path(&self) -> Option<&std::path::Path> {
        self.args.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl crate::ConfigPath for Remove {
    #[inline]
    fn config_path(&self) -> Option<&std::path::Path> {
        self.args.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl crate::ConfigPath for List {
    #[inline]
    fn config_path(&self) -> Option<&std::path::Path> {
        self.config_path.as_ref().map(PathBuf::as_path)
    }
}
