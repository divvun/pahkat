use anyhow::Result;
use std::{env, process::Command};

fn main() -> Result<()> {    
    let is_macos = env::var("CARGO_FEATURE_MACOS").ok().is_some();
    let is_windows = env::var("CARGO_FEATURE_WINDOWS").ok().is_some();
    let is_prefix = env::var("CARGO_FEATURE_PREFIX").ok().is_some();

    if !is_macos && !is_windows && !is_prefix {
        anyhow::bail!("Enable `macos`, `windows` or `prefix` feature.");
    }

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
