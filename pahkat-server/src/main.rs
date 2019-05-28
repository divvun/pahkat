use std::env;
use std::path::{Path, PathBuf};

#[macro_use]
extern crate diesel;

use actix_web::{web, App, HttpServer};
use clap::{crate_version, App as CliApp, AppSettings, Arg, SubCommand};
use log::{error, info, warn};

mod database;
mod handlers;
mod models;
mod watcher;

use handlers::{
    download_package, packages_index, packages_package_index, repo_index, virtuals_index,
    virtuals_package_index,
};
use pahkat_common::*;
use watcher::*;
use database::Database;

#[derive(Clone)]
pub struct ServerState {
    path: PathBuf,
    bind: String,
    port: String,
    database: Database,
}

fn run_server(path: &Path, bind: &str, port: &str) {
    let system = actix::System::new("páhkat-server");

    let database = match Database::new("db.sqlite3") {
        Ok(database) => database,
        Err(e) => {
            panic!("Failed to create database: {}", e);
        }
    };

    let state = ServerState {
        path: path.to_path_buf(),
        bind: bind.to_string(),
        port: port.to_string(),
        database,
    };

    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .service(web::resource("/index.json").route(web::get().to(repo_index)))
            .service(web::resource("/packages/index.json").route(web::get().to(packages_index)))
            .service(
                web::resource("/packages/{packageId}/index.json")
                    .route(web::get().to(packages_package_index)),
            )
            .service(
                web::resource("/packages/{packageId}/download")
                    .route(web::get().to(download_package)),
            )
            .service(web::resource("/virtuals/index.json").route(web::get().to(virtuals_index)))
            .service(
                web::resource("/virtuals/{packageId}/index.json")
                    .route(web::get().to(virtuals_package_index)),
            )
    })
    .bind(&format!("{}:{}", bind, port))
    .expect(&format!("Can not bind to {}:{}", bind, port))
    .start();

    let _ = system.run();
}

fn main() {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let matches = CliApp::new("Páhkat server")
        .version(crate_version!())
        .author("Rostislav Raykov <rostislav@technocreatives.com>")
        .about("Páhkat server implementation")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("run")
                .about("Run the server")
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
                ),
        )
        .get_matches();

    match matches.subcommand() {
        ("run", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let path: &Path = matches
                .value_of("path")
                .map_or(&current_dir, |v| Path::new(v));
            let bind: &str = matches.value_of("bind").map_or("127.0.0.1", |v| v);
            let port: &str = matches.value_of("port").map_or("8000", |v| v);

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

            run_server(path, bind, port);
        }
        _ => {}
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
