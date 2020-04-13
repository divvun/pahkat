use std::path::Path;

use pahkat_client::{package_store::InstallTarget, PackageStore};

pub fn status(
    store: &dyn PackageStore,
    packages: &Vec<String>,
    target: InstallTarget,
) -> Result<(), anyhow::Error> {
    if packages.is_empty() {
        println!("No packages specified.");
        return Ok(());
    }

    for id in packages {
        let (package_key, _) = match store.find_package_by_id(id) {
            Some(v) => v,
            None => {
                println!("{}: not found", &id);
                continue;
            }
        };
        match store.status(&package_key, target) {
            Ok(x) => println!("{}: {:?}", &package_key, x),
            Err(x) => println!("{}: {:?}", &package_key, x),
        }
    }

    Ok(())
}
