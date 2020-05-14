#[cfg(windows)]
pub mod windows;
pub mod selfupdate;

use anyhow::{anyhow, Result};
use log::{info, warn};
use pahkat_client::{
    config::RepoRecord, package_store::SharedStoreConfig, PackageKey, PackageStatus, PackageStore,
};
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

