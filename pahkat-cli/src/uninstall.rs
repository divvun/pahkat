use std::path::Path;

use pahkat_client::{package_store::InstallTarget, PackageStore};

pub fn uninstall(
    store: &dyn PackageStore,
    packages: &Vec<String>,
    target: InstallTarget,
) -> Result<(), anyhow::Error> {
    for id in packages {
        let pkg_key = store
            .find_package_by_id(id)
            .map(|x| x.0)
            .ok_or_else(|| anyhow::anyhow!("Could not find package for: `{}`", id))?;
        println!("Uninstalling {}", &pkg_key);
        let status = store.uninstall(&pkg_key, target)?;
        println!("{:?}", status);
    }
    Ok(())
}
