use std::path::Path;
use std::sync::Arc;

use futures::stream::StreamExt;

use crate::{Platform, cli::command::PackageSpec};
use pahkat_client::{
    package_store::InstallTarget,
    transaction::{PackageAction, PackageTransaction},
    DownloadEvent, PackageKey, PackageStore,
};

pub(crate) async fn install<'a>(
    store: Arc<dyn PackageStore>,
    packages: &'a Vec<PackageSpec>,
    target: InstallTarget,
    args: &'a crate::Args,
) -> Result<(), anyhow::Error> {
    let keys: Vec<PackageKey> = packages
        .iter()
        .map(|PackageSpec { id, version } | {
            let mut key: PackageKey = store
                .find_package_by_id(&id)
                .map(|x| x.0)
                .ok_or_else(|| anyhow::anyhow!("Could not find package for: `{}`", id))?;

            if let Some(platform) = args.platform() {
                key.query.platform = Some(platform.to_string());
            }

            if let Some(version) = version {
                key.query.version = Some(version.to_string());
            }

            Ok(key)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // for key in keys.iter() {
    //     // let pb = indicatif::ProgressBar::new(0);
    //     // pb.set_style(indicatif::ProgressStyle::default_bar()
    //     //     .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
    //     //     .progress_chars("=>-"));
    //     // pb.set_prefix(&key.id);

    //     let progress = Box::new(move |cur, max| {
    //         // pb.set_length(max);
    //         // pb.set_position(cur);

    //         // if cur >= max {
    //         //     pb.finish_and_clear();
    //         // }
    //         true
    //     });

    //     let _ = store.download(&key, progress)?;
    // }

    let transaction = PackageTransaction::new(
        Arc::clone(&store),
        keys.iter()
            .map(|x| PackageAction::install(x.clone(), target.clone()))
            .collect(),
    )?;

    for record in transaction.actions().iter() {
        let id = record.action.id.clone();
        let mut download = store.download(&record.action.id);

        // TODO: handle cancel here

        println!("Downloading {}", id);

        while let Some(event) = download.next().await {
            match event {
                DownloadEvent::Error(e) => {
                    println!("Error: {}", e);
                    return Ok(());
                }
                DownloadEvent::Progress((current, total)) => {
                    println!("Progress: {}/{}", current, total);
                }
                DownloadEvent::Complete(_) => {
                    println!("Complete");
                }
            }
        }
    }

    let (canceler, mut tx) = transaction.process();

    while let Some(event) = tx.next().await {
        let mut is_completed = false;
        use pahkat_client::transaction::TransactionEvent;

        // TODO: handle cancel here

        match event {
            TransactionEvent::Installing(id) => {
                println!("Installing: {}", id);
            }
            TransactionEvent::Uninstalling(id) => {
                println!("Uninstalling: {}", id);
            }
            TransactionEvent::Progress(id, msg) => {
                println!("Progress: {} {}", id, msg);
            }
            TransactionEvent::Error(id, err) => {
                println!("Error: {} {}", id, err);
                return Ok(());
            }
            TransactionEvent::Complete => {
                println!("Complete!");
                is_completed = true;
            }
        }
    }
    // transaction
    //     .process(|key, event| {
    //         println!("{:?}: {:?}", &key, &event);
    //         true
    //     })
    //     .join()
    //     .unwrap()?;
    Ok(())
}
