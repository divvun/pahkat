use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Serialize, Deserialize)]
struct Upload {
    #[structopt(short, long)]
    pub url: String,
    #[structopt(short, long)]
    pub version: pahkat_types::package::Version,
    #[structopt(short, long)]
    pub platform: String,
    #[structopt(short, long)]
    pub arch: Option<String>,
    #[structopt(short, long)]
    pub channel: Option<String>,
    #[structopt(short = "P", long)]
    pub payload_meta_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct PackageUpdateRequest {
    pub version: pahkat_types::package::Version,
    pub platform: String,
    pub arch: Option<String>,
    pub channel: Option<String>,
    pub payload: pahkat_types::payload::Payload,
}

#[derive(StructOpt)]
enum Args {
    Payload(pahkat_types::payload::Payload),
    Upload(Upload),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::from_args();

    match args {
        Args::Payload(payload) => {
            println!("{}", toml::to_string_pretty(&payload)?);
        }
        Args::Upload(upload) => {
            let auth = std::env::var("PAHKAT_API_KEY")?;

            let payload = std::fs::read_to_string(upload.payload_meta_path)?;
            let payload: pahkat_types::payload::Payload = toml::from_str(&payload)?;

            let json = PackageUpdateRequest {
                version: upload.version,
                platform: upload.platform,
                arch: upload.arch,
                channel: upload.channel,
                payload,
            };

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
