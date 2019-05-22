use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use clap::{crate_version, App as CliApp, AppSettings, Arg, SubCommand};

mod watcher;

use pahkat_common::*;
use watcher::*;

fn read_file(path: &str) -> std::io::Result<String> {
    let file = File::open(path)?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents)?;

    Ok(contents)
}

#[derive(Clone)]
struct ServerState {
    path: PathBuf,
    bind: String,
    port: String,
}

fn repo_index(state: web::Data<ServerState>) -> impl Responder {
    let mut repo_index_path = state.path.clone();

    repo_index_path.push("index.json");

    match read_file(
        repo_index_path
            .to_str()
            .expect("Cannot convert path to string"),
    ) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            eprintln!("Error while reading repo index file: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn packages_index(state: web::Data<ServerState>) -> impl Responder {
    let mut packages_index_path = state.path.clone();

    packages_index_path.push("packages");
    packages_index_path.push("index.json");

    match read_file(
        packages_index_path
            .to_str()
            .expect("Cannot convert path to string"),
    ) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            eprintln!("Error while reading packages index file: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn packages_package_index(
    state: web::Data<ServerState>,
    path: web::Path<String>,
) -> impl Responder {
    let package_id = path.clone();

    let mut packages_package_index_path = state.path.clone();

    packages_package_index_path.push("packages");
    packages_package_index_path.push(package_id);
    packages_package_index_path.push("index.json");
    let index_path_str = packages_package_index_path
        .to_str()
        .expect("Cannot convert path to string");

    match read_file(index_path_str) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            eprintln!(
                "Error while reading packages package index {}: {:?}",
                index_path_str, e
            );
            HttpResponse::NotFound().finish()
        }
    }
}

fn virtuals_index(state: web::Data<ServerState>) -> impl Responder {
    let mut virtuals_index_path = state.path.clone();

    virtuals_index_path.push("virtuals");
    virtuals_index_path.push("index.json");

    match read_file(
        virtuals_index_path
            .to_str()
            .expect("Cannot convert path to string"),
    ) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            eprintln!("Error while reading virtuals index file: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn virtuals_package_index(
    state: web::Data<ServerState>,
    path: web::Path<String>,
) -> impl Responder {
    let package_id = path.clone();

    let mut virtuals_package_index_path = state.path.clone();

    virtuals_package_index_path.push("virtuals");
    virtuals_package_index_path.push(package_id);
    virtuals_package_index_path.push("index.json");
    let index_path_str = virtuals_package_index_path
        .to_str()
        .expect("Cannot convert path to string");

    match read_file(index_path_str) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            eprintln!(
                "Error while reading virtuals package index {}: {:?}",
                index_path_str, e
            );
            HttpResponse::NotFound().finish()
        }
    }
}

fn run_server(path: &Path, bind: &str, port: &str) {
    let system = actix::System::new("páhkat-server");

    let state = ServerState {
        path: path.to_path_buf(),
        bind: bind.to_string(),
        port: port.to_string(),
    };

    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .service(web::resource("/index.json").route(web::get().to(repo_index)))
            .service(web::resource("/packages/index.json").route(web::get().to(packages_index)))
            .service(
                web::resource("/packages//{packageId}/index.json")
                    .route(web::get().to(packages_package_index)),
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

    println!("Running on port {} bound to {}", port, bind);
    let _ = system.run();
}

fn main() {
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
                        Err(error) => eprintln!("Failed to update watcher: {:?}", error),
                        Ok(ref events) if events.len() > 0 => {
                            println!(
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
        println!("Info {}", msg);
    }

    fn generating(&self, thing: &str) {
        println!("Generating {}", thing);
    }

    fn writing(&self, thing: &str) {
        println!("Writing {}", thing);
    }

    fn inserting(&self, id: &str, version: &str) {
        println!("Inserting {} {}", id, version);
    }

    fn error(&self, thing: &str) {
        eprintln!("Error {}", thing);
    }

    fn warn(&self, thing: &str) {
        println!("Warning {}", thing);
    }
}
