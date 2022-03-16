use anyhow::Result;

fn main() -> Result<()> {
    println!("cargo:rustc-env=GIT_VERSION={}", git_version::git_version!(args = ["--always", "--tags", "--dirty"]));
    println!("cargo:rustc-env=CARGO_TARGET_TRIPLE={}", std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string()));
    fbs_build::compile_fbs("../pahkat-types/src/index.fbs")
}
