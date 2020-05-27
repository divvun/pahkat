extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use std::path::Path;
use std::{env, fs};

use clap::{crate_authors, crate_description, crate_version, App as CliApp, Arg, ArgMatches};
use config::TomlConfig;
use failure::ResultExt;
use log::{error, info, warn};
use pahkat_common::ProgressOutput;
use server::run_server;
use std::path::PathBuf;
use watcher::Watcher;

mod config;
mod handlers;
mod server;
mod watcher;
use failure::Error;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let matches = CliApp::new("Páhkat server")
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .arg(
            Arg::with_name("path")
                .value_name("PATH")
                .help("The repository root directory (default: current working directory)")
                .short("p")
                .long("path")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("bind")
                .value_name("BIND")
                .help("The address which the server to listen to (default: 127.0.0.1)")
                .long("bind")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .value_name("PORT")
                .help("The port which the server to listen to (default: 8000)")
                .long("port")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Set a custom TOML config file")
                .takes_value(true),
        )
        .get_matches();

    let current_dir = &env::current_dir().unwrap();
    let path: &Path = matches
        .value_of("path")
        .map_or(&current_dir, |v| Path::new(v));
    let bind: &str = matches.value_of("bind").map_or("127.0.0.1", |v| v);
    let port: &str = matches.value_of("port").map_or("8080", |v| v);

    let mut watcher = Watcher::new(path).expect("Failed to start file watcher");

    let output = ConsoleOutput;

    std::thread::spawn(move || {
        let watcher_interval = std::time::Duration::from_millis(2000);
        loop {
            match watcher.update() {
                Err(error) => error!("Failed to update watcher: {:?}", error),
                Ok(ref events) if !events.is_empty() => {
                    info!(
                        "Watcher reports {} event(s) since last update",
                        events.len()
                    );
                    pahkat_common::repo_index(Path::new(watcher.path()), &output);
                    // todo: repo_ops calls need improved error handling to support:
                    // match repo_ops::repo_index(&path, &output) {
                    //     Err(error) => eprintln!("Failed to re-index pahkat repo at {}: {:?}", watcher.path(), error),
                    //     Ok(_) => println!("Successfully re-indexed pahkat repo at {}", watcher.path()),
                    // }
                }
                _ => {}
            }
            std::thread::sleep(watcher_interval);
        }
    });

    run_server(get_config(&matches)?, path, bind, port);

    Ok(())
}

fn get_config(matches: &ArgMatches<'_>) -> Result<TomlConfig, Error> {
    if let Some(config_file) = matches.value_of("config") {
        let config =
            fs::read_to_string(&config_file).expect(&format!("Failed to open {}", config_file));
        let config: TomlConfig =
            toml::from_str(&config).expect(&format!("Failed to convert {} to TOML", config_file));

        Ok(config)
    } else {
        Ok(TomlConfig {
            artifacts_dir: PathBuf::from(
                env::var("ARTIFACTS_DIR").with_context(|_| "ARTIFACTS_DIR not set")?,
            ),
            url_prefix: env::var("URL_PREFIX").with_context(|_| "URL_PREFIX not set")?,
            db_path: env::var("DATABASE_URL").ok(),
        })
    }
}

struct ConsoleOutput;

impl ProgressOutput for ConsoleOutput {
    fn info(&self, msg: &str) {
        info!("Info: {}", msg);
    }

    fn generating(&self, msg: &str) {
        info!("Generating {}", msg);
    }

    fn writing(&self, msg: &str) {
        info!("Writing {}", msg);
    }

    fn inserting(&self, id: &str, version: &str) {
        info!("Inserting {} {}", id, version);
    }

    fn error(&self, msg: &str) {
        error!("Error: {}", msg);
    }

    fn warn(&self, msg: &str) {
        warn!("Warning: {}", msg);
    }
}
