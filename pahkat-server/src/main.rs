use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::Infallible, path::PathBuf, sync::Arc};
use warp::http::StatusCode;
use warp::Filter;

use structopt::StructOpt;

#[derive(Serialize, Deserialize)]
struct PackageUpdateRequest {
    pub version: pahkat_types::package::Version,
    pub platform: String,
    pub arch: Option<String>,
    pub channel: Option<String>,
    pub payload: pahkat_types::payload::Payload,
}

#[derive(Debug, thiserror::Error)]
enum PackageUpdateError {
    #[error("Invalid API token")]
    Unauthorized,

    #[error("Unsupported repository identifier.")]
    UnsupportedRepo,

    #[error("{0}")]
    RepoError(#[from] pahkat_repomgr::package::update::Error),
}

impl warp::reject::Reject for PackageUpdateError {}

impl warp::reply::Reply for PackageUpdateError {
    fn into_response(self) -> warp::reply::Response {
        let msg = format!("{}", self);
        let code = match self {
            PackageUpdateError::Unauthorized => StatusCode::from_u16(403).unwrap(),
            PackageUpdateError::UnsupportedRepo => StatusCode::from_u16(400).unwrap(),
            PackageUpdateError::RepoError(_) => StatusCode::from_u16(500).unwrap(),
        };
        warp::reply::with_status(msg, code).into_response()
    }
}

async fn process_package_update_request(
    config: Arc<Config>,
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

    use std::borrow::Cow;

    let inner_req = pahkat_repomgr::package::update::Request::builder()
        .repo_path(repo_path.into())
        .id(package_id.into())
        .platform(req.platform.into())
        .arch(req.arch.map(|x| Cow::Owned(x)))
        .version(Cow::Owned(req.version))
        .channel(req.channel.map(|x| Cow::Owned(x)))
        .payload(Cow::Owned(req.payload))
        .url(None)
        .build();

    match pahkat_repomgr::package::update::update(inner_req) {
        Ok(_) => Ok(Box::new(format!("Success"))),
        Err(e) => Ok(Box::new(PackageUpdateError::RepoError(e))),
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    api_token: String,
    repos: HashMap<String, PathBuf>,
}

mod filters {
    use super::Config;
    use std::convert::Infallible;
    use std::sync::Arc;
    use warp::Filter;

    pub fn config(
        config: &Arc<Config>,
    ) -> impl Filter<Extract = (Arc<Config>,), Error = Infallible> + Clone {
        let config = Arc::clone(config);
        warp::any().map(move || Arc::clone(&config))
    }
}

#[derive(StructOpt)]
struct Args {
    #[structopt(short, long)]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let args = Args::from_args();

    let config = std::fs::read_to_string(&args.config_path)?;
    let config: Arc<Config> = Arc::new(toml::from_str(&config)?);

    log::info!("Repos: {:#?}", &config.repos);

    let package_update = warp::any()
        .and(warp::filters::method::patch())
        .and(filters::config(&config))
        .and(warp::path::param::<String>())
        .and(warp::path("packages"))
        .and(warp::path::param::<String>())
        .and(warp::body::json())
        .and(warp::header::<String>("authorization"))
        .and_then(process_package_update_request)
        .with(warp::log("pahkat_server::update_pkg"));

    let index = warp::any()
        .and(warp::path("/"))
        .map(|| "Hello!");

    warp::serve(package_update.or(index))
        .run(([127, 0, 0, 1], 3030))
        .await;

    Ok(())
}
