use anyhow::Result;
use std::process::Command;

fn main() -> Result<()> {
    let output = String::from_utf8(
        Command::new("git")
            .args(&["describe", "--always", "--tags", "--dirty"])
            .output()?
            .stdout,
    )?;
    println!("cargo:rustc-env=GIT_VERSION={}", output.trim());
    println!(
        "cargo:rustc-env=CARGO_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string())
    );
    fbs_build::compile_fbs("../pahkat-types/src/index.fbs")
}
