use anyhow::Result;

fn main() -> Result<()> {
    butte_build::compile_fbs("../pahkat-types/src/index.fbs")
}
