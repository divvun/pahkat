pub(crate) mod repo;

use crate::cli::constants::*;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Repo {
    #[structopt(template(SUB_TEMPLATE))]
    Add(repo::Add),
    #[structopt(template(SUB_TEMPLATE))]
    Remove(repo::Remove),
    #[structopt(template(SUBN_TEMPLATE))]
    List(repo::List),
}

impl crate::ConfigPath for Repo {
    #[inline]
    fn config_path(&self) -> Option<&std::path::Path> {
        match self {
            Repo::Add(x) => x.config_path(),
            Repo::Remove(x) => x.config_path(),
            Repo::List(x) => x.config_path(),
        }
    }
}
