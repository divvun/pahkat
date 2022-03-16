use anyhow::Result;
use vergen::{Config, SemverKind};

fn main() -> Result<()> {
    let mut config = Config::default();
    *config.git_mut().semver_kind_mut() = SemverKind::Lightweight;

    vergen::vergen(config)?;
    fbs_build::compile_fbs("../pahkat-types/src/index.fbs")
}
