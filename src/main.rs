#[macro_use]
extern crate clap;
extern crate bahkat;
extern crate bahkat_client;

use clap::{App, AppSettings, SubCommand};
use bahkat::types::*;
use bahkat_client::*;

fn main() {
    let matches = App::new("Báhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Bákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("list")
                .about("List packages in repository.")
        )
        .get_matches();

    match matches.subcommand() {
        ("list", Some(matches)) => {
            let repo = Repository::from_url("http://localhost:8000").unwrap();
            let mut packages: Vec<&Package> = repo.packages().values().collect();
            packages.sort_unstable_by(|a, b| a.id.cmp(&b.id));
            for pkg in packages {
                println!("{} {} ({}) — {}", pkg.id,
                    pkg.version,
                    pkg.name.get("en").unwrap_or(&"???".to_owned()),
                    pkg.description.get("en").unwrap_or(&"???".to_owned())
                );
            }
        },
        _ => {}
    }
}