use anyhow::Result;

fn main() -> Result<()> {
    fbs_build::compile_fbs("../pahkat-types/src/index.fbs")
}
