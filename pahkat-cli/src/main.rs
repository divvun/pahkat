#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;

use clap::{App, AppSettings, Arg, SubCommand};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use termcolor::Color;

mod cli;

use cli::*;
use pahkat_common::ld_type;
use pahkat_common::*;
use pahkat_types::*;

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

fn request_package_data(cur_dir: &Path) -> Option<Package> {
    let package_id = prompt_line("Package identifier", "").unwrap();

    if cur_dir
        .join(&format!("{}/index.json", &package_id))
        .exists()
    {
        progress(
            Color::Red,
            "Error",
            &format!("Package {} already exists; aborting.", &package_id),
        )
        .unwrap();
        return None;
    }

    let en_name = prompt_line("Name", "").unwrap();
    let mut name = BTreeMap::new();
    name.insert("en".to_owned(), en_name);

    let en_description = prompt_line("Description", "").unwrap();
    let mut description = BTreeMap::new();
    description.insert("en".to_owned(), en_description);

    let author = prompt_line("Author", "").unwrap();
    let license = prompt_line("License", "").unwrap();

    let version = prompt_line("Version", "0.1.0").unwrap();
    let category = prompt_line("Category", "").unwrap();

    println!("Package languages are languages the installed package supports.");
    let languages: Vec<String> = prompt_line("Package languages (comma-separated)", "en")
        .unwrap()
        .split(',')
        .map(|x| x.trim().to_owned())
        .collect();

    println!("Supported platforms: android, ios, linux, macos, windows");
    println!(
        "Specify platform support like \"windows\" or with version guards \"windows >= 8.1\"."
    );
    let platform_vec: Vec<String> = prompt_line("Platforms (comma-separated)", OS)
        .unwrap()
        .split(',')
        .map(|x| x.trim().to_owned())
        .collect();
    let platform = parse_platform_list(&platform_vec);

    Some(Package {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Package"),
        id: package_id,
        name,
        description,
        authors: vec![author],
        license,
        version,
        category,
        languages,
        platform,
        dependencies: Default::default(),
        virtual_dependencies: Default::default(),
        installer: None,
    })
}

fn request_virtual_data(cur_dir: &Path) -> Option<Virtual> {
    let package_id = prompt_line("Package identifier", "").unwrap();

    if cur_dir
        .join(&format!("{}/index.json", &package_id))
        .exists()
    {
        progress(
            Color::Red,
            "Error",
            &format!("Package {} already exists; aborting.", &package_id),
        )
        .unwrap();
        return None;
    }

    let en_name = prompt_line("Name", "").unwrap();
    let mut name = BTreeMap::new();
    name.insert("en".to_owned(), en_name);

    let version = prompt_line("Version", "0.1.0").unwrap();

    let en_description = prompt_line("Description", "").unwrap();
    let mut description = BTreeMap::new();
    description.insert("en".to_owned(), en_description);

    let en_help = prompt_line("Help", "").unwrap();
    let mut help = BTreeMap::new();
    help.insert("en".to_owned(), en_help);

    let opts = &[
        "Windows Registry Key",
        "macOS Package (ie, installed .pkg)",
        "macOS Path (ie, to an app)",
    ];

    let target = match prompt_select("Which target?", opts, 0) {
        0 => VirtualTarget::WindowsRegistryKey(RegistryKey {
            _type: ld_type!("RegistryKey"),
            name: prompt_line("Key name", "").unwrap(),
            path: prompt_line("Key path", "").unwrap(),
        }),
        1 => VirtualTarget::MacOSPackage(MacOSPackageRef {
            _type: ld_type!("MacOSPackageRef"),
            pkg_id: prompt_line("Package identifier", "").unwrap(),
            min_build: None,
            max_build: None,
            min_version: None,
            max_version: None,
        }),
        2 => VirtualTarget::MacOSPath(MacOSPathRef {
            _type: ld_type!("MacOSPathRef"),
            app_paths: vec![prompt_line("App path", "").unwrap()],
            min_build: None,
            max_build: None,
            min_version: None,
            max_version: None,
        }),
        _ => panic!("ohno"),
    };

    Some(Virtual {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Virtual"),
        id: package_id,
        name,
        description,
        // authors: vec![author],
        // license,
        version,
        help,
        url: None,
        target
        // category,
        // languages,
        // platform,
        // dependencies: Default::default(),
        // virtual_dependencies: Default::default(),
        // installer: None,
    })
}

fn input_repo_data() -> Repository {
    let base = {
        let mut base = String::new();
        while base == "" {
            let b = prompt_line("Base URL", "")
                .map(|b| {
                    if !base.ends_with('/') {
                        format!("{}/", b)
                    } else {
                        b
                    }
                })
                .unwrap();

            if url::Url::parse(&b).is_ok() {
                base = b;
            } else {
                progress(Color::Red, "Error", "Invalid URL.").unwrap();
            }
        }
        base
    };

    let en_name = prompt_line("Name", "Repository").unwrap();
    let mut name = BTreeMap::new();
    name.insert("en".to_owned(), en_name);

    let en_description = prompt_line("Description", "").unwrap();
    let mut description = BTreeMap::new();
    description.insert("en".to_owned(), en_description);

    let filters = &["category", "language"];
    let primary_filter = filters[prompt_select("Primary Filter", filters, 0)].to_string();

    let channels = {
        let mut r: Vec<String> = vec![];
        while r.is_empty() {
            r = prompt_multi_select("Channels", &["stable", "beta", "alpha", "nightly"]);
            if r.is_empty() {
                progress(
                    Color::Red,
                    "Error",
                    "No channels selected; please select at least one.",
                )
                .unwrap();
            }
        }
        r
    };

    let default_channel = if channels.len() == 1 {
        channels[0].to_string()
    } else {
        let i = prompt_select(
            "Default channel",
            &channels.iter().map(|x| x.as_ref()).collect::<Vec<_>>(),
            0,
        );
        channels[i].to_string()
    };

    let mut categories = BTreeMap::new();
    categories.insert("en".to_owned(), BTreeMap::new());

    Repository {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Repository"),
        agent: RepositoryAgent::default(),
        base,
        name,
        description,
        primary_filter,
        default_channel,
        channels,
        categories,
        linked_repositories: vec![],
    }
}

fn validate_repo(path: &Path) -> bool {
    match open_repo(&path) {
        Ok(_) => true,
        Err(e) => {
            match e {
                OpenIndexError::FileError(_) => {
                    progress(
                        Color::Red,
                        "Error",
                        "Cannot generate outside of a repository; aborting.",
                    )
                    .unwrap();
                }
                OpenIndexError::JsonError(e) => {
                    progress(Color::Red, "Error", &format!("JSON error: {}", e)).unwrap();
                    progress(
                        Color::Red,
                        "Error",
                        "Cannot parse repository JSON; aborting.",
                    )
                    .unwrap();
                }
            }
            false
        }
    }
}

fn virtual_init(output_dir: &Path, channel: Option<&str>) {
    if !validate_repo(output_dir) {
        return;
    }

    let virtual_data = match request_virtual_data(output_dir) {
        Some(v) => v,
        None => {
            return;
        }
    };

    let json = serde_json::to_string_pretty(&virtual_data).unwrap();

    println!("\n{}\n", json);

    if prompt_question("Save index.json", true) {
        let virtual_dir = output_dir.join(&format!("virtuals/{}", &virtual_data.id));
        fs::create_dir(&virtual_dir).unwrap();
        let mut file = File::create(&virtual_dir.join(index_fn(channel))).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        file.write_all(&[b'\n']).unwrap();
    }
}

fn package_init(output_dir: &Path, channel: Option<&str>) {
    if !validate_repo(output_dir) {
        return;
    }

    let pkg_data = match request_package_data(output_dir) {
        Some(v) => v,
        None => {
            return;
        }
    };

    let json = serde_json::to_string_pretty(&pkg_data).unwrap();

    println!("\n{}\n", json);

    if prompt_question("Save index.json", true) {
        let package_dir = output_dir.join(&format!("packages/{}", &pkg_data.id));
        fs::create_dir(&package_dir).unwrap();
        let mut file = File::create(&package_dir.join(index_fn(channel))).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        file.write_all(&[b'\n']).unwrap();
    }
}

fn repo_init<T: ProgressOutput>(cur_dir: &Path, output: &T) {
    if open_repo(&cur_dir).is_ok() {
        // progress(Color::Red, "Error", "Repo already exists; aborting.").unwrap();
        output.error("Repo already exists; aborting.");
        return;
    }

    if cur_dir.join("packages").exists() {
        output.error("A file or directory named 'packages' already exists; aborting.");
        // progress(Color::Red, "Error", "A file or directory named 'packages' already exists; aborting.").unwrap();
        return;
    }

    if cur_dir.join("virtuals").exists() {
        output.error("A file or directory named 'virtuals' already exists; aborting.");
        // progress(Color::Red, "Error", "A file or directory named 'virtuals' already exists; aborting.").unwrap();
        return;
    }

    let repo_data = input_repo_data();
    let json = serde_json::to_string_pretty(&repo_data).unwrap();

    println!("\n{}\n", json);

    if !prompt_question("Save index.json and generate repo directories?", true) {
        return;
    }

    if !cur_dir.exists() {
        fs::create_dir(&cur_dir).unwrap();
    }

    let mut file = File::create(cur_dir.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();

    fs::create_dir(cur_dir.join("packages")).unwrap();
    fs::create_dir(cur_dir.join("virtuals")).unwrap();

    repo_index(&cur_dir, output);
}

fn package_tarball_installer(
    file_path: &Path,
    channel: Option<&str>,
    force_yes: bool,
    tarball: &str,
    url: &str,
    size: usize,
) {
    let mut pkg = match open_package(&file_path, channel) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(
                Color::Red,
                "Error",
                "Package does not exist or is invalid; aborting",
            )
            .unwrap();
            return;
        }
    };

    let installer_file = File::open(tarball).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let installer_index = TarballInstaller {
        _type: ld_type!("TarballInstaller"),
        url: url.to_owned(),
        size: installer_size,
        installed_size: size,
    };

    pkg.installer = Some(Installer::Tarball(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();

    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    let mut file = File::create(file_path.join(index_fn(channel))).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();
}

fn package_macos_installer(
    file_path: &Path,
    channel: Option<&str>,
    version: &str,
    force_yes: bool,
    installer: &str,
    targets: Vec<&str>,
    pkg_id: &str,
    url: &str,
    size: usize,
    requires_reboot: bool,
    requires_uninst_reboot: bool,
) {
    let mut pkg = match open_package(&file_path, channel) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(
                Color::Red,
                "Error",
                "Package does not exist or is invalid; aborting",
            )
            .unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let target_results: Vec<Result<InstallTarget, &str>> = targets
        .iter()
        .map(|x| x.parse::<InstallTarget>().map_err(|_| *x))
        .collect();

    let target_errors: Vec<&str> = target_results
        .iter()
        .filter(|x| x.is_err())
        .map(|x| x.err().unwrap())
        .collect();

    if !target_errors.is_empty() {
        progress(
            Color::Red,
            "Error",
            &format!("Invalid targets supplied: {}", &target_errors.join(", ")),
        )
        .unwrap();
        return;
    }

    let targets: std::collections::BTreeSet<InstallTarget> = target_results
        .iter()
        .filter(|x| x.is_ok())
        .map(|x| x.unwrap())
        .collect();

    let installer_index = MacOSInstaller {
        _type: ld_type!("MacOSInstaller"),
        url: url.to_owned(),
        pkg_id: pkg_id.to_owned(),
        targets,
        requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None,
    };

    pkg.version = version.to_owned();
    pkg.installer = Some(Installer::MacOS(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();

    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    // TODO Check dir exists
    let mut file = File::create(file_path.join(index_fn(channel))).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();
}

fn package_windows_installer(
    file_path: &Path,
    channel: Option<&str>,
    force_yes: bool,
    product_code: &str,
    installer: &str,
    type_: Option<&str>,
    args: Option<&str>,
    uninst_args: Option<&str>,
    url: &str,
    size: usize,
    requires_reboot: bool,
    requires_uninst_reboot: bool,
) {
    let mut pkg = match open_package(&file_path, channel) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(
                Color::Red,
                "Error",
                "Package does not exist or is invalid; aborting",
            )
            .unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let installer_index = WindowsInstaller {
        _type: ld_type!("WindowsInstaller"),
        url: url.to_owned(),
        installer_type: type_.map(|x| x.to_owned()),
        args: args.map(|x| x.to_owned()),
        uninstall_args: uninst_args.map(|x| x.to_owned()),
        product_code: product_code.to_owned(),
        requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None,
    };

    pkg.installer = Some(Installer::Windows(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();

    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    let mut file = File::create(file_path.join(index_fn(channel))).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();
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
        _ => {}
    }
}
