#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;

use std::env;
use std::fs::File;
use std::path::Path;

use clap::{App, AppSettings, Arg, SubCommand};
use reqwest::multipart;
use termcolor::Color;

use cli::progress;
use commands::{
    db::create_user,
    installer::{package_macos_installer, package_tarball_installer, package_windows_installer},
    package_init, repo_init, virtual_init,
};

use pahkat_common::{repo_index, ProgressOutput, UploadParams};
use pahkat_types::{Downloadable, Installer};

mod cli;
mod commands;

struct StderrOutput;

impl ProgressOutput for StderrOutput {
    fn info(&self, msg: &str) {
        progress(Color::Cyan, "Info", msg).unwrap();
    }

    fn generating(&self, thing: &str) {
        progress(Color::Green, "Generating", thing).unwrap();
    }

    fn writing(&self, thing: &str) {
        progress(Color::Green, "Writing", thing).unwrap();
    }

    fn inserting(&self, id: &str, version: &str) {
        progress(Color::Yellow, "Inserting", &format!("{} {}", id, version)).unwrap();
    }

    fn error(&self, thing: &str) {
        progress(Color::Red, "Error", thing).unwrap();
    }

    fn warn(&self, thing: &str) {
        progress(Color::Magenta, "Warning", thing).unwrap();
    }
}

fn main() {
    let matches = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Páhkat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("repo")
            .about("Repository-related subcommands")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .arg(Arg::with_name("path")
                .value_name("PATH")
                .help("The repository root directory (default: current working directory)")
                .short("p")
                .long("path")
                .takes_value(true)
            )
            .subcommand(
                SubCommand::with_name("init")
                    .about("Initialise a Páhkat repository in the current working directory")
            )
            .subcommand(
                SubCommand::with_name("index")
                    .about("Regenerate packages and virtuals indexes from source indexes")
            )
        )
        .subcommand(
            SubCommand::with_name("package")
                .about("Package related functionality")
                .subcommand(
                    SubCommand::with_name("init")
                    .about("Initialise a package in the specified repository")
                    .arg(Arg::with_name("path")
                        .value_name("PATH")
                        .help("The repository root directory (default: current working directory)")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("channel")
                        .value_name("CHANNEL")
                        .help("The channel to use (default: repository default)")
                        .short("C")
                        .long("channel")
                        .takes_value(true)
                    )
                )
        )
        .subcommand(
            SubCommand::with_name("virtual")
                .about("Virtual package related functionality")
                .subcommand(
                    SubCommand::with_name("init")
                    .about("Initialise a virtual in the specified repository")
                    .arg(Arg::with_name("path")
                        .value_name("PATH")
                        .help("The repository root directory (default: current working directory)")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("channel")
                        .value_name("CHANNEL")
                        .help("The channel to use (default: repository default)")
                        .short("C")
                        .long("channel")
                        .takes_value(true)
                    )
                )
        )
        .subcommand(
            SubCommand::with_name("installer")
                .about("Inject installer data into package index")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .arg(Arg::with_name("path")
                    .value_name("PATH")
                    .help("The package index root directory (default: current working directory)")
                    .short("p")
                    .long("path")
                    .takes_value(true)
                )
                .subcommand(SubCommand::with_name("macos")
                    .about("Inject macOS .pkg installer data into package index")
                    .arg(Arg::with_name("package")
                        .value_name("PKG")
                        .help("The package file (.pkg)")
                        .short("i")
                        .long("package")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("version")
                        .value_name("VERSION")
                        .help("Package version")
                        .short("v")
                        .long("version")
                        .takes_value(true)
                        .required(true))
                    .arg(Arg::with_name("pkg-id")
                        .value_name("PKGID")
                        .help("The bundle identifier for the installed package (eg, com.example.package)")
                        .short("c")
                        .long("pkgid")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("targets")
                        .value_name("TARGETS")
                        .help("The supported targets for installation, comma-delimited (options: system, user)")
                        .short("o")
                        .long("targets")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("requires-reboot")
                        .help("Installer requires reboot after installation")
                        .short("r")
                        .long("reboot")
                    )
                    .arg(Arg::with_name("requires-uninst-reboot")
                        .help("Uninstaller requires reboot after installation")
                        .short("R")
                        .long("uninst-reboot")
                    )
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("The URL where the installer will be downloaded from")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installed-size")
                        .value_name("SIZE")
                        .help("The size on disk when the package is installed")
                        .short("s")
                        .long("size")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("skip-confirmation")
                        .help("Skip confirmation step")
                        .short("y")
                        .long("yes")
                    ))
                .subcommand(SubCommand::with_name("windows")
                    .about("Inject Windows installer data into package index")
                    .arg(Arg::with_name("product-code")
                        .value_name("PRODUCT_CODE")
                        .help("The product code that identifies the installer in the registry")
                        .short("c")
                        .long("code")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installer")
                        .value_name("INSTALLER")
                        .help("The installer .msi/.exe")
                        .short("i")
                        .long("installer")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("type")
                        .value_name("TYPE")
                        .help("Type of installer to autoconfigure silent install and uninstall (supported: msi, inno)")
                        .short("t")
                        .long("type")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("args")
                        .value_name("ARGS")
                        .help("Arguments to installer for it to run silently")
                        .short("a")
                        .long("args")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("uninst-args")
                        .value_name("ARGS")
                        .help("Arguments to uninstaller for it to run silently")
                        .short("A")
                        .long("uninst-args")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("requires-reboot")
                        .help("Installer requires reboot after installation")
                        .short("r")
                        .long("reboot")
                    )
                    .arg(Arg::with_name("requires-uninst-reboot")
                        .help("Uninstaller requires reboot after installation")
                        .short("R")
                        .long("uninst-reboot")
                    )
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("The URL where the installer will be downloaded from")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installed-size")
                        .value_name("SIZE")
                        .help("The size on disk when the package is installed")
                        .short("s")
                        .long("size")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("skip-confirmation")
                        .help("Skip confirmation step")
                        .short("y")
                        .long("yes")
                    ))
                .subcommand(SubCommand::with_name("tarball")
                    .about("Inject tarball install data into package index")
                    .arg(Arg::with_name("tarball")
                        .value_name("TARBALL")
                        .help("The 'installer' tarball (.txz)")
                        .short("i")
                        .long("tarball")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("The URL where the installer will be downloaded from")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installed-size")
                        .value_name("SIZE")
                        .help("The size on disk when the package is installed")
                        .short("s")
                        .long("size")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("skip-confirmation")
                        .help("Skip confirmation step")
                        .short("y")
                        .long("yes")
                    ))
        )
        .subcommand(SubCommand::with_name("database")
            .about("Access the database")
            .subcommand(SubCommand::with_name("user")
                .about("Manage users")
                .subcommand(SubCommand::with_name("create")
                    .about("Create a user")
                    .arg(Arg::with_name("username")
                        .value_name("USERNAME")
                        .help("The user name")
                        .short("n")
                        .long("username")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("token")
                        .value_name("TOKEN")
                        .help("The API access token for package uploads")
                        .short("t")
                        .long("token")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("db_path")
                    .value_name("DB_PATH")
                    .help("The path to the database")
                    .short("d")
                    .long("db")
                    .takes_value(true)
                )
                )
            )
        )
        .subcommand(SubCommand::with_name("upload")
            .about("Upload a package patch")
            .arg(Arg::with_name("url")
                 .help("repo url")
                 .index(1)
                 .required(true)
            )
            .arg(Arg::with_name("token")
                .value_name("TOKEN")
                .help("bearer token for an authorized user")
                .short("t")
                .long("token")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("package")
                .value_name("PKG-ID")
                .help("package id")
                .short("p")
                .long("package")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("channel")
                .value_name("CHANNEL")
                .help("package channel")
                .short("c")
                .long("channel")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("version")
                .value_name("VERSION")
                .help("new package installer version")
                .short("v")
                .long("version")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("installer")
                .value_name("INSTALLER")
                .help("json file for the package installer metadata")
                .short("i")
                .long("installer")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("file")
                .value_name("FILE")
                .help("installer file executable to update the package with")
                .short("f")
                .long("file")
                .takes_value(true)
            )
        )
        .get_matches();

    let output = StderrOutput;

    match matches.subcommand() {
        ("package", Some(matches)) => match matches.subcommand() {
            ("init", Some(matches)) => {
                let current_dir = &env::current_dir().unwrap();
                let path: &Path = matches
                    .value_of("path")
                    .map_or(&current_dir, |v| Path::new(v));
                let channel = matches.value_of("channel");
                package_init(&path, channel)
            }
            _ => {}
        },
        ("virtual", Some(matches)) => match matches.subcommand() {
            ("init", Some(matches)) => {
                let current_dir = &env::current_dir().unwrap();
                let path: &Path = matches
                    .value_of("path")
                    .map_or(&current_dir, |v| Path::new(v));
                let channel = matches.value_of("channel");
                virtual_init(&path, channel)
            }
            _ => {}
        },
        ("installer", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let path: &Path = matches
                .value_of("path")
                .map_or(&current_dir, |v| Path::new(v));

            match matches.subcommand() {
                ("macos", Some(matches)) => {
                    let installer = matches.value_of("package").unwrap();
                    let channel = matches.value_of("channel");
                    let version = matches.value_of("version").unwrap();
                    let targets: Vec<&str> =
                        matches.value_of("targets").unwrap().split(',').collect();
                    let pkg_id = matches.value_of("pkg-id").unwrap();
                    let url = matches.value_of("url").unwrap();
                    let size = matches
                        .value_of("installed-size")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    let requires_reboot = matches.is_present("requires-reboot");
                    let requires_uninst_reboot = matches.is_present("requires-uninst-reboot");
                    let skip_confirm = matches.is_present("skip-confirmation");

                    package_macos_installer(
                        path,
                        channel,
                        version,
                        skip_confirm,
                        installer,
                        targets,
                        pkg_id,
                        url,
                        size,
                        requires_reboot,
                        requires_uninst_reboot,
                    );
                }
                ("windows", Some(matches)) => {
                    let product_code = matches.value_of("product-code").unwrap();
                    let channel = matches.value_of("channel");
                    let type_ = matches.value_of("type");
                    let installer = matches.value_of("installer").unwrap();
                    let args = matches.value_of("args");
                    let uninstall_args = matches.value_of("uninst-args");
                    let url = matches.value_of("url").unwrap();
                    let size = matches
                        .value_of("installed-size")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    let requires_reboot = matches.is_present("requires-reboot");
                    let requires_uninst_reboot = matches.is_present("requires-uninst-reboot");
                    let skip_confirm = matches.is_present("skip-confirmation");

                    package_windows_installer(
                        path,
                        channel,
                        skip_confirm,
                        product_code,
                        installer,
                        type_,
                        args,
                        uninstall_args,
                        url,
                        size,
                        requires_reboot,
                        requires_uninst_reboot,
                    );
                }
                ("tarball", Some(matches)) => {
                    let tarball = matches.value_of("tarball").unwrap();
                    let channel = matches.value_of("channel");
                    let url = matches.value_of("url").unwrap();
                    let size = matches
                        .value_of("installed-size")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    let skip_confirm = matches.is_present("skip-confirmation");

                    package_tarball_installer(path, channel, skip_confirm, tarball, url, size);
                }
                _ => {}
            }
        }
        ("repo", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let path: &Path = matches
                .value_of("path")
                .map_or(&current_dir, |v| Path::new(v));

            match matches.subcommand() {
                ("init", _) => repo_init(&path, &output),
                ("index", _) => repo_index(&path, &output),
                _ => {}
            }
        }
        ("database", Some(matches)) => match matches.subcommand() {
            ("user", Some(matches)) => match matches.subcommand() {
                ("create", Some(matches)) => {
                    let username = matches.value_of("username").unwrap();
                    let token = matches.value_of("token").unwrap();
                    let db_path = matches.value_of("db_path");

                    create_user(username, token, db_path.map(|s| s.to_string()))
                }
                _ => {}
            },
            _ => {}
        },
        ("upload", Some(matches)) => {
            let repo_url = matches.value_of("url").unwrap();

            let token = matches.value_of("token").unwrap();
            let package_id = matches.value_of("package").unwrap();
            let channel = matches.value_of("channel").unwrap();
            let installer_file = matches.value_of("installer").unwrap();
            let version = matches.value_of("version").unwrap();

            // Only required if installer url is "pahkat:payload"
            let payload_file = matches.value_of("file");

            let patch_url = format!("{}/packages/{}", repo_url, package_id);

            let file = File::open(installer_file).expect("the installer file to be valid");
            let mut installer: Installer =
                serde_json::from_reader(file).expect("the json in the installer file to be valid");

            let client = reqwest::Client::new();

            let installer_url = installer.url();

            let mut form = multipart::Form::new();

            if installer_url == "pahkat:payload" || payload_file.is_some() {
                match payload_file {
                    Some(file) => {
                        let installer_file =
                            File::open(file).expect("Installer could not be opened.");
                        let meta = installer_file.metadata().unwrap();
                        let installer_size = meta.len() as usize;
                        match installer {
                            Installer::Windows(ref mut installer) => {
                                installer.url = "pahkat:payload".to_string();
                                installer.size = installer_size;
                            }
                            Installer::MacOS(ref mut installer) => {
                                installer.url = "pahkat:payload".to_string();
                                installer.size = installer_size;
                            }
                            _ => panic!("Installer type not supported"),
                        };

                        form = form
                            .file("payload", file)
                            .expect("payload file to be valid");
                    }
                    None => {
                        panic!("A file must be provided if installer url is pahkat:payload");
                    }
                }
            }

            let upload_params = UploadParams {
                channel: channel.to_owned(),
                version: version.to_owned(),
                installer: installer.clone(),
            };

            form = form.text("params", serde_json::to_string(&upload_params).unwrap());

            let mut response = client
                .patch(&patch_url)
                .bearer_auth(token)
                .multipart(form)
                .send()
                .unwrap();

            let text = response.text().unwrap();

            println!("Response: {:?}, body: {}", &response, &text);
        }
        _ => {}
    }
}
