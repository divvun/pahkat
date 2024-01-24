use anyhow::{Context, Result};
use pahkat_types::LangTagMap;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use structopt::clap::arg_enum;
use structopt::StructOpt;

#[derive(StructOpt)]
enum Args {
    Release(Release),
    Upload(Upload),
}

#[derive(StructOpt, Serialize, Deserialize)]
struct Upload {
    #[structopt(short, long)]
    pub url: String,

    #[structopt(short = "P", long)]
    pub release_meta: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(long)]
    pub metadata_json: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(long, requires("package-type"))]
    pub manifest_toml: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(long, possible_values = &PackageType::variants(), case_insensitive = true)]
    pub package_type: Option<PackageType>,
}

arg_enum! {
    #[derive(Debug, Serialize, Deserialize)]
    enum PackageType {
        Speller,
        // to be expanded with grammar checkers and other types in the future
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, structopt::StructOpt)]
pub struct Release {
    #[structopt(short, long)]
    pub version: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(short, long)]
    pub channel: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[structopt(long)]
    pub authors: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(short, long)]
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(long)]
    pub license_url: Option<String>,

    #[structopt(flatten)]
    pub target: pahkat_types::payload::Target,

    // loaded from metadata file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(skip)]
    pub name: Option<LangTagMap<String>>,

    // loaded from metadata file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[structopt(skip)]
    pub description: Option<LangTagMap<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::from_args();

    match args {
        Args::Release(release) => {
            println!("{}", toml::to_string_pretty(&release)?);
        }
        Args::Upload(upload) => {
            let auth =
                std::env::var("PAHKAT_API_KEY").context("could not read env PAHKAT_API_KEY")?;

            let release = std::fs::read_to_string(upload.release_meta)?;
            let mut release: Release = toml::from_str(&release)?;

            if let Some(path) = upload.metadata_json {
                names_and_descs(&mut release, &path)
                    .with_context(|| format!("could not read metadata from {path:?}"))?;
            }

            if let Some(path) = upload.manifest_toml {
                let package_type = upload.package_type.unwrap();
                names_and_descs_toml(&mut release, &path, &package_type)
                    .with_context(|| format!("could not read metadata from {path:?}"))?;
            }

            let client = reqwest::Client::new();
            let mut retries = 0;

            while retries <= 3 {
                let response = client
                    .patch(&upload.url)
                    .json(&release)
                    .header("authorization", format!("Bearer {}", auth))
                    .send()
                    .await?;

                match response.error_for_status_ref() {
                    Ok(_) => {
                        println!("Response: {}", response.text().await?);
                        break;
                    }
                    Err(err) => {
                        eprintln!("Errored with status {}", err.status().unwrap());
                        match response.text().await {
                            Ok(v) => eprintln!("{}", v),
                            Err(_) => {}
                        }
                        match err.status().unwrap().as_u16() {
                            500..=599 => {
                                eprintln!("Retrying");
                                retries += 1;
                                continue;
                            }
                            _ => std::process::exit(1),
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn names_and_descs(release: &mut Release, metadata_json: &Path) -> Result<()> {
    let metadata = std::fs::read_to_string(metadata_json)?;
    // assume json is like: {en: {name: "", description: ""}}
    let metadata: BTreeMap<String, BTreeMap<String, String>> = serde_json::from_str(&metadata)?;
    // convert to {name: {en: ""}, description: {en: ""}}
    let (mut names, mut descriptions) = (LangTagMap::<String>::new(), LangTagMap::<String>::new());
    metadata.iter().for_each(|(lang, map)| {
        if let Some(name) = map.get("name") {
            names.insert(lang.clone(), name.clone());
        }
        if let Some(description) = map.get("description") {
            descriptions.insert(lang.clone(), description.clone());
        }
    });
    release.name = Some(names);
    release.description = Some(descriptions);

    // DEBUG
    // dbg!(&release);
    // println!("{}", serde_json::to_string(&release)?);
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    speller: SpellerMeta,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SpellerMeta {
    name: BTreeMap<String, String>,
    description: BTreeMap<String, String>,
}

fn names_and_descs_toml(
    release: &mut Release,
    metadata_toml: &Path,
    package_type: &PackageType,
) -> Result<()> {
    let manifest = std::fs::read_to_string(metadata_toml)?;
    let manifest: Manifest = toml::from_str(&manifest)?;
    let metadata = match package_type {
        PackageType::Speller => manifest.speller,
    };

    release.name = Some(metadata.name);
    release.description = Some(metadata.description);

    // DEBUG
    // dbg!(&release);
    // println!("{}", serde_json::to_string(&release)?);
    Ok(())
}
