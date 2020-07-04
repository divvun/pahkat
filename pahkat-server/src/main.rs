use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::time::Duration;
use std::{collections::HashMap, convert::Infallible, path::PathBuf, sync::Arc};
use warp::http::StatusCode;
use warp::Filter;

use structopt::StructOpt;

use std::process::{Command, ExitStatus, Output};

pub struct Subversion {
    path: PathBuf,
}

impl Subversion {
    fn cleanup(&self) -> Result<Output, Output> {
        let output = Command::new("svn")
            .args(&["cleanup", "--remove-unversioned"])
            .current_dir(&self.path)
            .output()
            .unwrap();

        if output.status.success() {
            Ok(output)
        } else {
            Err(output)
        }
    }

    fn revert(&self) -> Result<Output, Output> {
        let output = Command::new("svn")
            .args(&["revert", "-R", "."])
            .current_dir(&self.path)
            .output()
            .unwrap();

        if output.status.success() {
            Ok(output)
        } else {
            Err(output)
        }
    }

    fn update(&self) -> Result<Output, Output> {
        let output = Command::new("svn")
            .arg("up")
            .current_dir(&self.path)
            .output()
            .unwrap();

        if output.status.success() {
            Ok(output)
        } else {
            Err(output)
        }
    }

    fn commit(
        &self,
        repo: &str,
        id: &str,
        username: &str,
        password: &str,
    ) -> Result<Output, Output> {
        let output = Command::new("svn")
            .args(&["commit", "-m"])
            .arg(format!("[CD] Package: {}, Repo: {}", id, repo))
            .args(&[
                format!("--username={}", username),
                format!("--password={}", password),
            ])
            .current_dir(&self.path)
            .output()
            .unwrap();

        if output.status.success() {
            Ok(output)
        } else {
            Err(output)
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PackageUpdateRequest {
    release: Release,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Release {
    pub version: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_url: Option<String>,

    pub target: pahkat_types::payload::Target,
}

#[derive(Debug, thiserror::Error)]
enum PackageUpdateError {
    #[error("Invalid API token")]
    Unauthorized,

    #[error("Unsupported repository identifier.")]
    UnsupportedRepo,

    #[error("{0}")]
    RepoError(#[from] pahkat_repomgr::package::update::Error),

    #[error("Invalid version provided")]
    VersionError(#[from] pahkat_types::package::version::Error),

    #[error("Indexing error")]
    IndexError,
}

impl warp::reject::Reject for PackageUpdateError {}

impl warp::reply::Reply for PackageUpdateError {
    fn into_response(self) -> warp::reply::Response {
        let msg = format!("{}", self);
        let code = match self {
            PackageUpdateError::Unauthorized => StatusCode::from_u16(403).unwrap(),
            PackageUpdateError::UnsupportedRepo => StatusCode::from_u16(400).unwrap(),
            PackageUpdateError::RepoError(_) => StatusCode::from_u16(500).unwrap(),
            PackageUpdateError::IndexError => StatusCode::from_u16(500).unwrap(),
            PackageUpdateError::VersionError(_) => StatusCode::from_u16(400).unwrap(),
        };
        warp::reply::with_status(msg, code).into_response()
    }
}

async fn process_package_update_request(
    config: Arc<Config>,
    svn: Arc<HashMap<String, Arc<Mutex<Subversion>>>>,
    repo_id: String,
    package_id: String,
    req: PackageUpdateRequest,
    auth_token: String,
) -> Result<Box<dyn warp::Reply>, Infallible> {
    if !auth_token.starts_with("Bearer ") {
        return Ok(Box::new(PackageUpdateError::Unauthorized));
    }

    let candidate = auth_token.split(" ").skip(1).next().unwrap();
    if candidate != config.api_token {
        return Ok(Box::new(PackageUpdateError::Unauthorized));
    }

    if !config.repos.contains_key(&repo_id) {
        return Ok(Box::new(PackageUpdateError::UnsupportedRepo));
    }

    let repo_path = &config.repos[&repo_id];

    log::info!("Waiting for lock on repository...");
    let svn = svn[&repo_id].lock().await;

    log::info!("Got lock!");

    loop {
        log::info!("Updating repository...");
        svn.revert().unwrap();
        svn.cleanup().unwrap();
        svn.update().unwrap();

        let version: pahkat_types::package::Version = match req.release.version.parse() {
            Ok(v) => v,
            Err(e) => return Ok(Box::new(PackageUpdateError::VersionError(e))),
        };

        let inner_req = pahkat_repomgr::package::update::Request::builder()
            .repo_path(svn.path.clone().into())
            .id(package_id.clone().into())
            .version(Cow::Owned(version))
            .channel(req.release.channel.as_ref().map(|x| Cow::Borrowed(&**x)))
            .target(Cow::Borrowed(&req.release.target))
            .url(None)
            .build();

        log::info!("Updating package...");
        match pahkat_repomgr::package::update::update(inner_req) {
            Ok(_) => {}
            Err(e) => return Ok(Box::new(PackageUpdateError::RepoError(e))),
        };

        log::info!("Updating index...");
        match pahkat_repomgr::repo::indexing::index(
            pahkat_repomgr::repo::indexing::Request::builder()
                .path(svn.path.clone().into())
                .build(),
        ) {
            Ok(_) => {}
            Err(e) => return Ok(Box::new(PackageUpdateError::IndexError)),
        };

        log::info!("Committing to repository...");
        match svn.commit(
            &repo_id,
            &package_id,
            &config.svn_username,
            &config.svn_password,
        ) {
            Ok(_) => break,
            Err(_) => {
                log::error!("Blocked from committing; retrying in 5 seconds.");
                tokio::time::delay_for(std::time::Duration::from_secs(5)).await;
                continue;
            }
        }
    }

    Ok(Box::new("Success"))
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    svn_username: String,
    svn_password: String,
    api_token: String,
    repos: HashMap<String, PathBuf>,
}

mod filters {
    use super::*;
    use std::collections::HashMap;
    use std::convert::Infallible;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use warp::Filter;

    pub fn config(
        config: &Arc<Config>,
    ) -> impl Filter<Extract = (Arc<Config>,), Error = Infallible> + Clone {
        let config = Arc::clone(config);
        warp::any().map(move || Arc::clone(&config))
    }

    pub fn svn(
        svn: &Arc<HashMap<String, Arc<Mutex<Subversion>>>>,
    ) -> impl Filter<Extract = (Arc<HashMap<String, Arc<Mutex<Subversion>>>>,), Error = Infallible> + Clone
    {
        let svn = Arc::clone(svn);
        warp::any().map(move || Arc::clone(&svn))
    }
}

#[derive(StructOpt)]
struct Args {
    #[structopt(short, long)]
    config_path: PathBuf,
}

use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let args = Args::from_args();

    let config = std::fs::read_to_string(&args.config_path)?;
    let config: Arc<Config> = Arc::new(toml::from_str(&config)?);

    let mut svn = HashMap::new();
    for (key, value) in config.repos.iter() {
        svn.insert(
            key.to_string(),
            Arc::new(Mutex::new(Subversion {
                path: std::fs::canonicalize(value.clone()).unwrap(),
            })),
        );
    }
    let svn = Arc::new(svn);

    log::info!("Repos: {:#?}", &config.repos);

    let package_update = warp::any()
        .and(warp::filters::method::patch())
        .and(filters::config(&config))
        .and(filters::svn(&svn))
        .and(warp::path::param::<String>())
        .and(warp::path("packages"))
        .and(warp::path::param::<String>())
        .and(warp::body::json())
        .and(warp::header::<String>("authorization"))
        .and_then(process_package_update_request)
        .with(warp::log("pahkat_server::update_pkg"));

    // let index = warp::any()
    //     .map(|| "Hello!");

    warp::serve(package_update)
        .run(([127, 0, 0, 1], 3030))
        .await;

    Ok(())
}
