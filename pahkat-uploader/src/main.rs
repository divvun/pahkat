use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Serialize, Deserialize)]
struct Upload {
    #[structopt(short, long)]
    pub url: String,
    #[structopt(short = "P", long)]
    pub release_meta_path: PathBuf,
}

#[derive(StructOpt)]
enum Args {
    Release(Release),
    Upload(Upload),
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::from_args();

    match args {
        Args::Release(release) => {
            println!("{}", toml::to_string_pretty(&release)?);
        }
        Args::Upload(upload) => {
            let auth = std::env::var("PAHKAT_API_KEY")?;

            let release = std::fs::read_to_string(upload.release_meta_path)?;
            let json: Release = toml::from_str(&release)?;

            let client = reqwest::Client::new();

            let response = client
                .patch(&upload.url)
                .json(&json)
                .header("authorization", format!("Bearer {}", auth))
                .send()
                .await?;

            match response.error_for_status_ref() {
                Ok(_) => {
                    println!("Response: {}", response.text().await?);
                }
                Err(err) => {
                    eprintln!("Errored with status {}", err.status().unwrap());
                    match response.text().await {
                        Ok(v) => eprintln!("{}", v),
                        Err(_) => {}
                    }
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
