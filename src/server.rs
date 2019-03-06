use actix_web::{server, actix::System, App, HttpResponse, Responder, State, http::Method, Path as WebPath, error::ErrorNotFound};
use clap::{AppSettings, App as CliApp, SubCommand, Arg, crate_version};
use std::env;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;

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
    port: String,
}

fn repo_index(state: State<ServerState>) -> impl Responder {
    let mut repo_index_path = state.path.clone();

    repo_index_path.push("index.json");
    
    read_file(repo_index_path.to_str().expect("Cannot convert path to string"))
}

fn packages_index(state: State<ServerState>) -> impl Responder {
    let mut packages_index_path = state.path.clone();

    packages_index_path.push("packages");
    packages_index_path.push("index.json");
    
    read_file(packages_index_path.to_str().expect("Cannot convert path to string"))
}

fn virtuals_index(state: State<ServerState>) -> impl Responder {
    let mut virtuals_index_path = state.path.clone();

    virtuals_index_path.push("virtuals");
    virtuals_index_path.push("index.json");
    
    read_file(virtuals_index_path.to_str().expect("Cannot convert path to string"))
}

fn virtuals_package_index(state: State<ServerState>, path: WebPath<String>) -> impl Responder {
    let package_id = path.clone();

    let mut virtuals_package_index_path = state.path.clone();

    virtuals_package_index_path.push("virtuals");
    virtuals_package_index_path.push(package_id);
    virtuals_package_index_path.push("index.json");
    
    match read_file(virtuals_package_index_path.to_str().expect("Cannot convert path to string")) {
        Ok(v) => Ok(v),
        Err(e) => {
            eprintln!("Error while reading virtuals package index: {:?}", e);
            Err(ErrorNotFound("Not found"))
        },
    }
}

fn run_server(path: &Path, port: &str) {
    let system = System::new("páhkat-server");

    let state = ServerState {
        path: path.to_path_buf(),
        port: port.to_string()
    };

    server::new(move || {
            App::with_state(state.clone())
                .resource("/index.json", |r| r.method(Method::GET).with(repo_index))
                .resource("/packages/index.json", |r| r.method(Method::GET).with(packages_index))
                .resource("/virtuals/index.json", |r| r.method(Method::GET).with(virtuals_index))
                .resource("/virtuals/{packageId}/index.json", |r| r.method(Method::GET).with(virtuals_package_index))
        })
        .bind(&format!("127.0.0.1:{}", port))
        .expect(&format!("Can not bind to port {}", port))
        .start();

    println!("Running on port {}", port);

    system.run();
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
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("port")
                        .value_name("PORT")
                        .help("The port which the server to listen to (default: 8000)")
                        .long("port")
                        .takes_value(true)
                )

        )
        .get_matches();

    match matches.subcommand() {
        ("run", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let path: &Path = matches.value_of("path")
                .map_or(&current_dir, |v| Path::new(v));
            let port: &str = matches.value_of("port")
                .map_or("8000", |v| v);

            run_server(path, port)
        }
        _ => {}
    }
}
