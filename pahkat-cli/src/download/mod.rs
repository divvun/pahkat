use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::pin_mut;
use futures::stream::StreamExt;

use pahkat_client::{DownloadEvent, PackageKey, PackageStore};

pub async fn download<'a>(
    store: Arc<dyn PackageStore>,
    packages: &'a Vec<String>,
    output_path: &'a Path,
) -> Result<(), anyhow::Error> {
    std::fs::create_dir_all(output_path)?;

    let keys: Vec<PackageKey> = packages
        .iter()
        .map(|id| {
            store
                .find_package_by_id(id)
                .map(|x| x.0)
                .ok_or_else(|| anyhow::anyhow!("Could not find package for: `{}`", id))
        })
        .collect::<Result<Vec<_>, _>>()?;

    println!("Preparing to download:");
    for key in keys.iter() {
        println!(" - {}", &key);
    }

    for key in keys {
        let pb = indicatif::ProgressBar::new(0);
        pb.set_style(indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("=>-"));
        pb.set_prefix(&key.id);

        // let progress = Box::new(move |cur, max| {
        //     // pb.set_length(max);
        //     // pb.set_position(cur);

        //     // if cur >= max {
        //     //     pb.finish_and_clear();
        //     // }
        //     true
        // });

        let mut download = store.download(&key);

        // pin_mut!(download);

        while let Some(event) = download.next().await {
            match event {
                DownloadEvent::Progress((current, total)) => {
                    pb.set_length(total);
                    pb.set_position(current);
                }
                DownloadEvent::Complete(pkg_path) => {
                    std::fs::copy(&pkg_path, output_path.join(pkg_path.file_name().unwrap()))?;
                    std::fs::remove_file(&pkg_path)?;
                    pb.finish();
                }
                _ => {}
            }
        }
    }
    Ok(())
}
