#![recursion_limit = "1024"]

pub mod client;
pub mod server;

use futures::stream::{StreamExt, TryStreamExt};
use log::{error, info, warn};
use pahkat_client::{
    config::RepoRecord, package_store::InstallTarget, PackageAction, PackageActionType, PackageKey,
    PackageStatus, PackageStore, PackageTransaction,
};
use parity_tokio_ipc::{Endpoint, SecurityAttributes};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use url::Url;

mod pb {
    tonic::include_proto!("/pahkat");
}

impl From<RepoRecord> for pb::RepoRecord {
    fn from(repo: RepoRecord) -> pb::RepoRecord {
        pb::RepoRecord {
            channel: repo.channel.unwrap_or_else(|| "".into()),
        }
    }
}

impl From<pb::PackageAction> for PackageAction {
    fn from(input: pb::PackageAction) -> PackageAction {
        PackageAction {
            id: PackageKey::try_from(&*input.id).unwrap(),
            action: PackageActionType::from_u8(input.action as u8),
            target: InstallTarget::from(input.target as u8),
        }
    }
}

impl From<PackageAction> for pb::PackageAction {
    fn from(input: PackageAction) -> pb::PackageAction {
        pb::PackageAction {
            id: input.id.to_string(),
            action: input.action.to_u8() as u32,
            target: input.target.to_u8() as u32,
        }
    }
}

impl From<pahkat_client::transaction::ResolvedAction> for pb::ResolvedAction {
    fn from(record: pahkat_client::transaction::ResolvedAction) -> Self {
        pb::ResolvedAction {
            action: Some(record.action.into()),
            name: record.descriptor.name.into_iter().collect(),
            version: record.release.version.to_string(),
        }
    }
}

impl From<pahkat_client::repo::LoadedRepository> for pb::LoadedRepository {
    fn from(value: pahkat_client::repo::LoadedRepository) -> pb::LoadedRepository {
        pb::LoadedRepository {
            index: Some(pb::loaded_repository::Index {
                url: value.info.repository.url.to_string(),
                channels: value.info.repository.channels,
                default_channel: value
                    .info
                    .repository
                    .default_channel
                    .unwrap_or_else(|| "".into()),
                name: value.info.name.into_iter().collect::<HashMap<_, _>>(),
                description: value
                    .info
                    .description
                    .into_iter()
                    .collect::<HashMap<_, _>>(),
                agent: Some(pb::loaded_repository::index::Agent {
                    name: value.info.agent.name,
                    version: value.info.agent.version,
                    url: value
                        .info
                        .agent
                        .url
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "".into()),
                }),
                landing_url: value
                    .info
                    .repository
                    .landing_url
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "".into()),
                linked_repositories: value
                    .info
                    .repository
                    .linked_repositories
                    .iter()
                    .map(|x| x.to_string())
                    .collect(),
                accepted_redirections: value
                    .info
                    .repository
                    .accepted_redirections
                    .iter()
                    .map(|x| x.to_string())
                    .collect(),
            }),
            meta: Some(pb::loaded_repository::Meta {
                channel: value.meta.clone().channel.unwrap_or_else(|| "".into()),
            }),
            packages_fbs: value.packages.to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
enum Notification {
    RebootRequired,
    RepositoriesChanged,
}

type Result<T> = std::result::Result<Response<T>, Status>;
type Stream<T> =
    Pin<Box<dyn futures::Stream<Item = std::result::Result<T, Status>> + Send + Sync + 'static>>;

struct Rpc {
    store: Arc<dyn PackageStore>,
    notifications: broadcast::Sender<Notification>,
    current_transaction: Arc<tokio::sync::Mutex<()>>,
}

#[tonic::async_trait]
impl pb::pahkat_server::Pahkat for Rpc {
    type NotificationsStream = Stream<pb::NotificationResponse>;
    type ProcessTransactionStream = Stream<pb::TransactionResponse>;

    async fn notifications(
        &self,
        _request: Request<pb::NotificationsRequest>,
    ) -> Result<Self::NotificationsStream> {
        let mut rx = self.notifications.subscribe();

        // log::info!("Peer: {:?}", _request.peer_cred());

        let stream = async_stream::try_stream! {
            while let response = rx.recv().await {
                match response {
                    Ok(response) => {
                        use pb::notification_response::ValueType;
                        match response {
                            Notification::RebootRequired => {
                                yield pb::NotificationResponse { value: ValueType::RebootRequired as i32 };
                            }
                            Notification::RepositoriesChanged => {
                                yield pb::NotificationResponse { value: ValueType::RepositoriesChanged as i32 };
                            }
                        }
                    },
                    Err(err) => {
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream) as Self::NotificationsStream))
    }

    async fn strings(&self, request: Request<pb::StringsRequest>) -> Result<pb::StringsResponse> {
        let pb::StringsRequest { language } = request.into_inner();

        let strings = self.store.strings(language).await;

        use pb::strings_response::MessageMap;

        Ok(Response::new(pb::StringsResponse {
            repos: strings
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        MessageMap {
                            tags: v.tags.into_iter().collect(),
                            channels: v.channels.into_iter().collect(),
                        },
                    )
                })
                .collect(),
        }))
    }

    async fn status(&self, request: Request<pb::StatusRequest>) -> Result<pb::StatusResponse> {
        let request = request.into_inner();
        let package_id = PackageKey::try_from(&*request.package_id)
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let result = pahkat_client::transaction::status_to_i8(
            self.store.status(&package_id, Default::default()),
        );
        Ok(Response::new(pb::StatusResponse {
            value: result.try_into().unwrap(),
        }))
    }

    async fn repository_indexes(
        &self,
        _request: Request<pb::RepositoryIndexesRequest>,
    ) -> Result<pb::RepositoryIndexesResponse> {
        let repos = self.store.repos();
        let repos = repos.read().unwrap();

        Ok(Response::new(pb::RepositoryIndexesResponse {
            repositories: repos
                .values()
                .map(|x| pb::LoadedRepository::from(x.clone()))
                .collect(),
        }))
    }

    async fn process_transaction(
        &self,
        request: Request<tonic::Streaming<pb::TransactionRequest>>,
    ) -> Result<Self::ProcessTransactionStream> {
        let mut request = request.into_inner();
        let store: Arc<dyn PackageStore> = Arc::clone(&self.store as _);
        let current_transaction = Arc::clone(&self.current_transaction);

        let (tx, rx) = mpsc::unbounded_channel();
        // Get messages
        tokio::spawn(async move {
            let mut has_requested = false;
            let mut has_cancelled = false;

            futures::pin_mut!(request);
            let (escape_catch_tx, _) = tokio::sync::broadcast::channel(1);

            'listener: loop {
                let escape_catch_tx = escape_catch_tx.clone();
                let mut rx = escape_catch_tx.subscribe();

                let request = tokio::select! {
                    request = request.message() => request,
                    _ = rx.recv() => break 'listener
                };

                let value = match request {
                    Ok(Some(v)) => match v.value {
                        Some(v) => v,
                        None => return
                    }
                    Err(err) => {
                        log::error!("{:?}", err);
                        return;
                    }
                    Ok(None) => return,
                };

                let request = match value {
                    pb::transaction_request::Value::Transaction(v) => {
                        if has_requested {
                            // Duplicate transaction requests on same pipe is an error.
                            // TODO: reply with an error message
                            return;
                        }
                        has_requested = true;
                        v
                    }
                    pb::transaction_request::Value::Cancel(_) => {
                        if has_requested {
                            // We can cancel this transaction as it is ours.
                            has_cancelled = true;
                        }

                        return;
                    }
                };

                let actions = request
                    .actions
                    .into_iter()
                    .map(|x| PackageAction::from(x))
                    .collect::<Vec<_>>();
                println!("{:?}", &actions);

                let transaction =
                    PackageTransaction::new(Arc::clone(&store) as _, actions).unwrap(); // .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

                let store = Arc::clone(&store);
                let current_transaction = Arc::clone(&current_transaction);

                let tx = tx.clone();

                tokio::spawn(async move {
                    // If there is a running transaction, we must block on this transaction and wait

                    log::debug!("Waiting for transaction lock…");
                    let _guard = current_transaction.lock().await;
                    log::debug!("Transaction lock attained.");

                    let tx1 = tx.clone();
                    let stream = async_stream::try_stream! {
                        let tx = tx1;
                        use pahkat_client::package_store::DownloadEvent;
                        use pb::transaction_response::*;

                        yield pb::TransactionResponse {
                            value: Some(Value::TransactionStarted(TransactionStarted {
                                actions: transaction.actions().iter().cloned().map(|x| x.into()).collect(),
                                is_reboot_required: transaction.is_reboot_required(),
                            }))
                        };

                        for record in transaction.actions().iter() {
                            let tx = tx.clone();
                            let id = record.action.id.clone();
                            let mut download = store.download(&record.action.id);

                            // TODO: handle cancel here

                            while let Some(event) = download.next().await {
                                match event {
                                    DownloadEvent::Error(e) => {
                                        yield pb::TransactionResponse {
                                            value: Some(Value::TransactionError(TransactionError {
                                                package_id: id.to_string(),
                                                error: format!("{}", e)
                                            }))
                                        };
                                        return;
                                    }
                                    DownloadEvent::Progress((current, total)) => {
                                        yield pb::TransactionResponse {
                                            value: Some(Value::DownloadProgress(DownloadProgress {
                                                package_id: id.to_string(),
                                                current,
                                                total,
                                            }))
                                        };
                                    }
                                    DownloadEvent::Complete(_) => {
                                        yield pb::TransactionResponse {
                                            value: Some(Value::DownloadComplete(DownloadComplete {
                                                package_id: id.to_string(),
                                            }))
                                        };
                                    }
                                }
                            }
                        }
                        log::trace!("Ending download stream");

                        let (canceler, mut tx_stream) = transaction.process();
                        let mut is_completed = false;

                        while let Some(event) = tx_stream.next().await {
                            use pahkat_client::transaction::TransactionEvent;

                            // TODO: handle cancel here

                            match event {
                                TransactionEvent::Installing(id) => {
                                    yield pb::TransactionResponse {
                                        value: Some(Value::InstallStarted(InstallStarted {
                                            package_id: id.to_string(),
                                        }))
                                    };
                                }
                                TransactionEvent::Uninstalling(id) => {
                                    yield pb::TransactionResponse {
                                        value: Some(Value::UninstallStarted(UninstallStarted {
                                            package_id: id.to_string(),
                                        }))
                                    };
                                }
                                TransactionEvent::Progress(id, msg) => {
                                    yield pb::TransactionResponse {
                                        value: Some(Value::TransactionProgress(TransactionProgress {
                                            package_id: id.to_string(),
                                            message: msg,
                                            current: 0,
                                            total: 0,
                                        }))
                                    };
                                }
                                TransactionEvent::Error(id, err) => {
                                    yield pb::TransactionResponse {
                                        value: Some(Value::TransactionError(TransactionError {
                                            package_id: id.to_string(),
                                            error: format!("{}", err),
                                        }))
                                    };

                                    return;
                                }
                                TransactionEvent::Complete => {
                                    yield pb::TransactionResponse {
                                        value: Some(Value::TransactionComplete(TransactionComplete {}))
                                    };
                                    is_completed = true;
                                }
                            }
                        }
                        log::trace!("Ending inner transaction stream");

                        if !is_completed {
                            yield pb::TransactionResponse {
                                value: Some(Value::TransactionError(TransactionError {
                                    package_id: "".to_string(),
                                    error: "user cancelled".to_string(),
                                }))
                            };
                        }
                        
                        log::trace!("Ending transaction stream");
                        return;
                    };

                    futures::pin_mut!(stream);

                    while let Some(value) = stream.next().await {
                        match tx.send(value) {
                            Ok(_) => {}
                            Err(err) => {
                                log::error!("{:?}", err);
                            }
                        }
                    }
                    log::trace!("Ending outer stream loop");
                    
                    // HACK: this lets us escape the stream.
                    match escape_catch_tx.send(()) {
                        Ok(_) => {},
                        Err(err) => {
                            log::error!("{:?}", err);
                        }
                    }
                });
            }

            log::trace!("Ended entire listener loop");
        });

        Ok(Response::new(Box::pin(rx) as Self::ProcessTransactionStream))
    }

    async fn set_repo(
        &self,
        request: tonic::Request<pb::SetRepoRequest>,
    ) -> Result<pb::SetRepoResponse> {
        let request = request.into_inner();
        let url =
            Url::parse(&request.url).map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let config = self.store.config();
        {
            let mut config = config.write().unwrap();
            let mut repos = config.repos_mut();

            let mut record = RepoRecord::default();

            if let Some(other_record) = request.settings {
                if other_record.channel != "" {
                    record.channel = Some(other_record.channel);
                }
            }

            repos
                .insert(url, record)
                .map_err(|e| Status::failed_precondition(format!("{}", e)))?;
        }

        self.store
            .force_refresh_repos()
            .await
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let _ = self.notifications.send(Notification::RepositoriesChanged);

        let mut config = config.read().unwrap();
        let mut repos = config.repos();

        Ok(tonic::Response::new(pb::SetRepoResponse {
            records: repos.iter().map(|(k, v)| (k.to_string(), v.to_owned().into())).collect(),
            error: "".into(),
        }))
    }

    async fn get_repo_records(
        &self,
        _request: tonic::Request<pb::GetRepoRecordsRequest>,
    ) -> Result<pb::GetRepoRecordsResponse> {
        let config = self.store.config();
        let config = config.read().unwrap();
        let repos = config.repos();
        
        Ok(tonic::Response::new(pb::GetRepoRecordsResponse {
            records: repos.iter().map(|(k, v)| (k.to_string(), v.to_owned().into())).collect(),
            error: "".into(),
        }))
    }

    async fn remove_repo(
        &self,
        request: tonic::Request<pb::RemoveRepoRequest>,
    ) -> Result<pb::RemoveRepoResponse> {
        let request = request.into_inner();

        let url =
            Url::parse(&request.url).map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let config = self.store.config();

        let is_success = {
            let mut config = config.write().unwrap();
            let mut repos = config.repos_mut();
            repos
                .remove(&url)
                .map_err(|e| Status::failed_precondition(format!("{}", e)))?
        };

        self.store
            .force_refresh_repos()
            .await
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;
            
        let _ = self.notifications.send(Notification::RepositoriesChanged);

        let mut config = config.read().unwrap();
        let mut repos = config.repos();

        Ok(tonic::Response::new(pb::RemoveRepoResponse {
            records: repos.iter().map(|(k, v)| (k.to_string(), v.to_owned().into())).collect(),
            error: "".into(),
        }))
    }
}

use std::path::Path;

#[inline(always)]
#[cfg(feature = "prefix")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config_path = config_path.ok_or_else(|| anyhow::anyhow!("No prefix path specified"))?;
    let store = pahkat_client::PrefixPackageStore::open(config_path)?;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        log::warn!("There are no repositories in the given config.");
    }

    Ok(store)
}

#[inline(always)]
#[cfg(feature = "macos")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config = match config_path {
        Some(v) => pahkat_client::Config::load(&v, pahkat_client::Permission::ReadWrite)?,
        None => pahkat_client::Config::load_default()?,
    };
    let store = pahkat_client::MacOSPackageStore::new(config).await;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        log::warn!("There are no repositories in the given config.");
    }

    Ok(store)
}

#[inline(always)]
#[cfg(feature = "windows")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config = match config_path {
        Some(v) => pahkat_client::Config::load(&v, pahkat_client::Permission::ReadWrite)?,
        None => pahkat_client::Config::load_default()?,
    };
    log::debug!("Loading config...");
    let store = pahkat_client::WindowsPackageStore::new(config).await;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        log::warn!("There are no repositories in the given config.");
    }

    Ok(store)
}

#[cfg(unix)]
#[inline(always)]
fn endpoint(path: &Path) -> std::result::Result<UnixListener, anyhow::Error> {
    use std::os::unix::fs::FileTypeExt;

    match std::fs::metadata(path) {
        Ok(v) => {
            log::warn!(
                "Unexpected file found at Unix socket path: {}",
                &path.display()
            );
            if v.file_type().is_socket() {
                std::fs::remove_file(&path)?;
                log::warn!("Deleted stale socket.");
            } else {
                log::error!("File is not a Unix socket, refusing to clean up automatically.");
                anyhow::bail!("Unexpected file at desired Unix socket path ({}) cannot be automatically cleaned up.", &path.display());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            log::error!("{}", &e);
            return Err(e.into());
        }
    };

    Ok(tokio::net::UnixListener::bind(&path).unwrap())
}

#[cfg(windows)]
#[inline(always)]
fn endpoint(path: &Path) -> std::result::Result<Endpoint, anyhow::Error> {
    Ok(Endpoint::new(path.to_str().unwrap().to_string()))
}

fn create_background_update_service(
    store: Arc<dyn PackageStore>,
    current_transaction: Arc<tokio::sync::Mutex<()>>,
) {
    const UPDATE_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes

    tokio::spawn(async move {
        // let current_transaction = Arc::clone(&current_transaction);
        // let store = Arc::clone(&store);

        use tokio::time::{self, Duration};
        let mut interval = time::interval(UPDATE_INTERVAL);

        'main: loop {
            interval.tick().await;

            time::delay_for(Duration::from_secs(10)).await;
            let _ = store.refresh_repos().await;

            log::info!("Running update check…");
            // match server::check_for_self_update(store.clone()) {
            //     Ok(server::SelfUpdateStatus::Recheck) | Ok(server::SelfUpdateStatus::Required) => {
            //         {
            //             let _guard = current_transaction.lock().await;
            //             let _ = store.refresh_repos().await;
            //         }

            //         if server::check_and_initiate_self_update(store.clone())
            //             .await
            //             .unwrap_or_default()
            //         {
            //             // Wait some time for the impending shutdown
            //             time::delay_for(Duration::from_secs(10)).await;
            //             continue;
            //         }
            //     }
            //     Err(e) => error!("self update error {:?}", e),
            //     _ => {}
            // }

            // Currently installed packages:
            log::debug!("Iterating through all known packages...");
            let updates = {
                let repos = store.repos();
                let repos = repos.read().unwrap();

                let mut updates = vec![];

                for (url, repo) in repos.iter() {
                    log::debug!("## Repo: {:?}", &url);
                    let statuses = store.all_statuses(url, pahkat_client::InstallTarget::System);

                    for (key, value) in statuses.into_iter() {
                        log::debug!(" - {:?}: {:?}", &key, &value);
                        if let Ok(PackageStatus::RequiresUpdate) = value {
                            updates.push((
                                pahkat_client::PackageKey {
                                    repository_url: url.clone(),
                                    id: key,
                                    query: Default::default(),
                                },
                                pahkat_client::InstallTarget::System,
                            ));
                        }
                    }
                }

                updates
            };

            log::debug!("Proposed updates: {:?}", &updates);

            if updates.is_empty() {
                log::info!("No updates found.");
                continue;
            }

            let actions = updates
                .into_iter()
                .map(|(package_key, target)| PackageAction::install(package_key, target))
                .collect::<Vec<_>>();

            log::debug!("Waiting for transaction lock…");
            let _guard = current_transaction.lock().await;
            log::debug!("Transaction lock attained.");

            let transaction = PackageTransaction::new(Arc::clone(&store) as _, actions).unwrap(); // .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

            for record in transaction.actions().iter() {
                let action = &record.action;
                // let tx = tx.clone();
                // let id = action.id.clone();
                let mut download = store.download(&action.id);

                // TODO: handle cancel here

                use pahkat_client::package_store::DownloadEvent;

                while let Some(event) = download.next().await {
                    match event {
                        DownloadEvent::Error(e) => {
                            log::error!("{:?}", &e);
                            continue 'main;
                        }
                        event => {
                            log::debug!("{:?}", &event);
                        }
                    };
                    //     DownloadEvent::Progress((current, total)) => {
                    //         yield pb::TransactionResponse {
                    //             value: Some(Value::DownloadProgress(DownloadProgress {
                    //                 package_id: id.to_string(),
                    //                 current,
                    //                 total,
                    //             }))
                    //         };
                    //     }
                    //     DownloadEvent::Complete(_) => {
                    //         yield pb::TransactionResponse {
                    //             value: Some(Value::DownloadComplete(DownloadComplete {
                    //                 package_id: id.to_string(),
                    //             }))
                    //         };
                    //     }
                    // }
                }
            }

            let (_canceler, mut stream) = transaction.process();

            futures::pin_mut!(stream);

            while let Some(message) = stream.next().await {
                log::trace!("{:?}", message);
            }

            log::debug!("Completed background transaction.");
        }
    });
}

#[cfg(unix)]
fn shutdown_handler(
    mut shutdown_rx: mpsc::UnboundedReceiver<()>,
    current_transaction: Arc<tokio::sync::Mutex<()>>,
) -> anyhow::Result<Pin<Box<dyn std::future::Future<Output = ()>>>, anyhow::Error> {

    let sigint_listener = signal(SignalKind::interrupt())?.into_future();
    let sigterm_listener = signal(SignalKind::terminate())?.into_future();
    let sigquit_listener = signal(SignalKind::quit())?.into_future();

    // SIGUSR1 and SIGUSR2 do nothing, just swallow events.
    let _sigusr1_listener = signal(SignalKind::user_defined1())?.into_future();
    let _sigusr2_listener = signal(SignalKind::user_defined2())?.into_future();

    Ok(Box::pin(async move {
        log::debug!("Created signal listeners for: SIGINT, SIGTERM, SIGQUIT, SIGUSR1, SIGUSR2.");

        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Shutdown signal received; gracefully shutting down.");
            }
            _ = sigint_listener => {
                log::info!("SIGINT received; gracefully shutting down.");
            }
            _ = sigterm_listener => {
                log::info!("SIGTERM received; gracefully shutting down.");
            }
            _ = sigquit_listener => {
                log::info!("SIGQUIT received; gracefully shutting down.");
            }
        };

        log::info!("Attempting to attain transaction lock...");
        current_transaction.lock().await;
        log::info!("Lock attained!");
        ()
    }))
}

#[cfg(windows)]
fn shutdown_handler(
    mut shutdown_rx: mpsc::UnboundedReceiver<()>,
    current_transaction: Arc<tokio::sync::Mutex<()>>,
) -> impl std::future::Future<Output = ()> {
    let ctrl_c = tokio::signal::ctrl_c();

    async move {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Shutdown signal received; gracefully shutting down.");
            }
            _ = ctrl_c => {
                log::info!("Ctrl+C received; gracefully shutting down.");
            }
        };

        log::info!("Attempting to attain transaction lock...");
        current_transaction.lock().await;
        log::info!("Lock attained!");
        ()
    }
}


#[cfg(unix)]
pub async fn start(
    path: &Path,
    config_path: Option<&Path>,
    shutdown_rx: mpsc::UnboundedReceiver<()>,
) -> std::result::Result<(), anyhow::Error> {
    let mut endpoint = endpoint(path)?;

    // let incoming = endpoint.incoming().map(|x| match x {
    //     Ok(v) => {
    //         // log::debug!("PEER: {:?}", &v.peer_cred());
    //         Ok(v)
    //     }
    //     Err(e) => Err(e),
    // }); //.expect("failed to open new socket");

    let store = store(config_path).await?;
    log::debug!("Created store.");

    let current_transaction = Arc::new(tokio::sync::Mutex::new(()));

    // Create the background updater
    // create_background_update_service(Arc::clone(&store), Arc::clone(&current_transaction));

    // Notifications
    let (notifications, mut notif_rx) = broadcast::channel(5);

    let rpc = Rpc {
        store: Arc::clone(&store),
        notifications,
        current_transaction: Arc::clone(&current_transaction),
    };

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(rpc))
        .serve_with_incoming_shutdown(
            endpoint.incoming().map_ok(StreamBox),
            shutdown_handler(shutdown_rx, Arc::clone(&current_transaction))?,
        )
        .await?;

    // drop(endpoint);

    log::info!("Cleaning up Unix socket at path: {}", &path.display());
    std::fs::remove_file(&path)?;

    log::info!("Shutdown complete!");
    Ok(())
}

#[cfg(windows)]
pub async fn start(
    path: &Path,
    config_path: Option<&Path>,
    shutdown_rx: mpsc::UnboundedReceiver<()>,
) -> std::result::Result<(), anyhow::Error> {
    let mut endpoint = endpoint(path)?;

    let incoming = endpoint.incoming()?;

    let store = store(config_path).await?;
    log::debug!("Created store.");

    let current_transaction = Arc::new(tokio::sync::Mutex::new(()));

    // Create the background updater
    // create_background_update_service(Arc::clone(&store), Arc::clone(&current_transaction));

    // Notifications
    let (notifications, mut notif_rx) = broadcast::channel(5);

    let rpc = Rpc {
        store: Arc::clone(&store),
        notifications,
        current_transaction: Arc::clone(&current_transaction),
    };

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(rpc))
        .serve_with_incoming_shutdown(
            incoming,
            shutdown_handler(shutdown_rx, Arc::clone(&current_transaction)),
        )
        .await?;

    // drop(endpoint);

    // log::info!("Cleaning up Unix socket at path: {}", &path.display());
    // std::fs::remove_file(&path)?;

    log::info!("Shutdown complete!");
    Ok(())
}

#[derive(Debug)]
pub struct StreamBox<T: AsyncRead + AsyncWrite>(pub T);

impl<T: AsyncRead + AsyncWrite> Connected for StreamBox<T> {}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for StreamBox<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for StreamBox<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
