use std::borrow::Cow;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use typed_builder::TypedBuilder;

#[non_exhaustive]
#[derive(Debug, Clone, TypedBuilder)]
pub struct Request<'a> {
    pub repo_path: Cow<'a, Path>,
    pub keep: u32,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct PartialRequest<'a> {
    #[builder(default)]
    pub repo_path: Option<&'a Path>,
    #[builder(default)]
    pub keep: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Provided path was invalid")]
    PathError(#[source] io::Error),

    #[error("Could not find repository at provided path")]
    NoRepo(#[from] FindRepoError),

    #[error("Could not read payload TOML file")]
    Io(#[from] std::io::Error),

    #[error("Could not read payload TOML file")]
    PayloadToml(#[from] toml::de::Error),

    #[error("Invalid input")]
    InvalidInput,
}

#[derive(Debug, thiserror::Error)]
pub enum FindRepoError {
    #[error("IO error")]
    Io(#[from] io::Error),

    #[error("No repository found for given path")]
    NotFound,
}

fn open_repo(path: &Path) -> Option<pahkat_types::repo::Repository> {
    let file = fs::read_to_string(path.join("index.toml")).ok()?;
    let repo: pahkat_types::repo::Repository = toml::from_str(&file).ok()?;
    Some(repo)
}

fn find_repo(path: &Path) -> Result<&Path, FindRepoError> {
    let mut path = path;

    if path.ends_with("index.toml") {
        path = path.parent().unwrap();
    }

    if let Some(_) = open_repo(path) {
        return Ok(path);
    }

    while let Some(parent) = path.parent() {
        path = parent;
        if let Some(_) = open_repo(path) {
            return Ok(path);
        }
    }

    Err(FindRepoError::NotFound)
}

impl<'a> crate::Request for Request<'a> {
    type Error = RequestError;
    type Partial = PartialRequest<'a>;

    fn new_from_user_input(partial: Self::Partial) -> Result<Self, Self::Error> {
        use dialoguer::Input;

        let repo_path = match partial.repo_path {
            Some(path) => Cow::Borrowed(path),
            None => Input::<String>::new()
                .default(
                    std::env::current_dir()
                        .ok()
                        .and_then(|x| x.to_str().map(str::to_string))
                        .unwrap_or_else(|| ".".into()),
                )
                .with_prompt("Repository Path")
                .interact()
                .map(|p| Cow::Owned(PathBuf::from(p)))
                .map_err(RequestError::PathError)?,
        };

        let _ = find_repo(&repo_path)?;

        let keep = match partial.keep {
            Some(keep) => keep,
            None => Input::<u32>::new()
                .default(1)
                .with_prompt("Releases to keep")
                .interact()?,
        };

        Ok(Request { repo_path, keep })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to read descriptor index: `{0}`")]
    ReadFailed(PathBuf, #[source] io::Error),

    #[error("Failed to read TOML file `{0}`")]
    ReadToml(PathBuf, #[source] toml::de::Error),

    #[error("Failed to create directory `{0}`")]
    DirCreateFailed(PathBuf, #[source] io::Error),

    #[error("Failed to write TOML file `{0}`")]
    WriteToml(PathBuf, #[source] io::Error),

    #[error("Failed to serialize TOML for `{0}`")]
    SerializeToml(PathBuf, #[source] toml::ser::Error),

    #[error("Could not find repository at provided path")]
    NoRepo(#[from] FindRepoError),
}

pub fn nuke_nightlies<'a>(request: Request<'a>) -> Result<(), Error>
{
    log::debug!("{:?}", request);

    let pkgs_dir = find_repo(&request.repo_path)?.join("packages");

    let pkgs_paths = std::fs::read_dir(pkgs_dir).unwrap();

    for pkg_path in pkgs_paths {
        let path = pkg_path.unwrap().path();
        if !path.is_dir() {
            continue;
        }

        let pkg_path = path.join("index.toml");

        let pkg_file = std::fs::read_to_string(&pkg_path)
            .map_err(|e| Error::ReadFailed(pkg_path.clone(), e))?;
        let mut descriptor: pahkat_types::package::Descriptor =
            toml::from_str(&pkg_file).map_err(|e| Error::ReadToml(pkg_path.clone(), e))?;

        descriptor.release = descriptor.release
        .into_iter()
        .scan(0, |state, release| {
            if let Some(channel) = &release.channel {
                if channel == "nightly" {
                    *state = *state + 1;
                }
            }

            Some((release, *state))
        })
        .filter(|(release, nightly_count)| {
            if let Some(channel) = &release.channel {
                if channel == "nightly" && *nightly_count > request.keep {
                    for url in release.target.iter().map(|t| t.payload.url().to_string()) {
                        println!("{}", url);
                    }

                    return false;
                }
            }

            return true;
        })
        .map(|(release, _)| release).collect();

        // Write the toml
        let data = toml::to_string_pretty(&descriptor)
            .map_err(|e| Error::SerializeToml(pkg_path.clone(), e))?;
        fs::write(&pkg_path, data).map_err(|e| Error::WriteToml(pkg_path.to_path_buf(), e))?;
        log::info!("Wrote descriptor to {}", pkg_path.display());
    }

    Ok(())
}
