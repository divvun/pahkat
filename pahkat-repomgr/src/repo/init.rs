use std::borrow::Cow;
use std::fs::{self, create_dir_all, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use typed_builder::TypedBuilder;
use url::Url;

use pahkat_types::{
    repo::{Agent, Index, RepoUrl, RepoUrlError, RepositoryData},
    LangTagMap,
};

#[non_exhaustive]
#[derive(Debug, Clone, TypedBuilder)]
pub struct Request<'a> {
    pub path: Cow<'a, Path>,
    pub url: Cow<'a, RepoUrl>,
    pub name: Cow<'a, str>,
    pub description: Cow<'a, str>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct PartialRequest<'a> {
    #[builder(default)]
    pub path: Option<&'a Path>,
    #[builder(default)]
    pub url: Option<&'a Url>,
    #[builder(default)]
    pub name: Option<&'a str>,
    #[builder(default)]
    pub description: Option<&'a str>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Provided path was invalid")]
    PathError(#[source] io::Error),

    #[error("Invalid input")]
    InvalidInput,

    #[error("Invalid URL")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Repository URL was not valid")]
    InvalidRepoUrl(#[from] RepoUrlError),
}

impl<'a> crate::Request for Request<'a> {
    type Error = RequestError;
    type Partial = PartialRequest<'a>;

    fn new_from_user_input(partial: Self::Partial) -> Result<Self, Self::Error> {
        use dialoguer::Input;

        let path = match partial.path {
            Some(path) => Cow::Borrowed(path),
            None => Input::<String>::new()
                .default(
                    std::env::current_dir()
                        .ok()
                        .and_then(|x| x.to_str().map(str::to_string))
                        .unwrap_or_else(|| ".".into()),
                )
                .with_prompt("Path")
                .interact()
                .map(|p| Cow::Owned(PathBuf::from(p)))
                .map_err(RequestError::PathError)?,
        };

        let url = match partial.url {
            Some(url) => {
                let url = RepoUrl::new(url.to_owned())?;
                Cow::Owned(url)
            }
            None => {
                let url = Input::<String>::new()
                    .with_prompt("Base URL")
                    .with_initial_text("https://")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?;
                let url = Url::parse(&url)?;
                let url = RepoUrl::new(url)?;
                Cow::Owned(url)
            }
        };

        let name = match partial.name {
            Some(name) => Cow::Borrowed(name),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Repo name (in English)")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        let description = match partial.description {
            Some(description) => Cow::Borrowed(description),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Repo description (in English)")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        Ok(Request {
            path,
            url,
            name,
            description,
        })
    }
}

pub fn create_agent() -> Agent {
    Agent::builder()
        .name("pahkat".to_string())
        .version(env!("CARGO_PKG_VERSION").into())
        .url(Some(
            Url::parse("https://github.com/divvun/pahkat/").unwrap(),
        ))
        .build()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create directory `{0}`")]
    DirCreateFailed(PathBuf, #[source] io::Error),

    #[error("Failed to write TOML file `{0}`")]
    WriteToml(PathBuf, #[source] io::Error),

    #[error("Failed to serialize TOML for `{0}`")]
    SerializeToml(PathBuf, #[source] toml::ser::Error),
}

fn write_index<T: Serialize>(path: &Path, index: &T) -> Result<(), Error> {
    let data = toml::to_string(index).map_err(|e| Error::SerializeToml(path.to_path_buf(), e))?;
    fs::write(&path, data).map_err(|e| Error::WriteToml(path.to_path_buf(), e))
}

pub fn init<'a>(request: Request<'a>) -> Result<(), Error> {
    // Create all the directories
    create_dir_all(&request.path)
        .map_err(|e| Error::DirCreateFailed(request.path.to_path_buf(), e))?;
    create_dir_all(&request.path.join("packages"))
        .map_err(|e| Error::DirCreateFailed(request.path.join("packages").to_path_buf(), e))?;
    create_dir_all(&request.path.join("strings"))
        .map_err(|e| Error::DirCreateFailed(request.path.join("strings").to_path_buf(), e))?;

    // Create empty repository index
    let mut name = LangTagMap::new();
    name.insert("en".into(), request.name.to_string());

    let mut description = LangTagMap::new();
    description.insert("en".into(), request.description.to_string());

    let data = RepositoryData::builder()
        .url(request.url.into_owned())
        .build();
    let index = Index::builder()
        .repository(data)
        .agent(create_agent())
        .name(name)
        .description(description)
        .build();

    let repo_index_path = request.path.join("index.toml");
    write_index(&repo_index_path, &index)?;

    Ok(())
}
