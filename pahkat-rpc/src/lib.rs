#![recursion_limit = "1024"]

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::stream::{StreamExt, TryStreamExt};
use parity_tokio_ipc::{Endpoint, SecurityAttributes};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::{transport::Server, Request, Response, Status, Streaming};

use pahkat_client::{
    PackageAction, PackageActionType, PackageKey, PackageStore, PackageTransaction,
    package_store::InstallTarget,
};

mod pb {
    tonic::include_proto!("/pahkat");
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
    Test,
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

        log::info!("Peer: {:?}", _request.peer_cred());

        let stream = async_stream::try_stream! {
            while let response = rx.recv().await {
                match response {
                    Ok(response) => {
                        match response {
                            Notification::Test => {
                                yield pb::NotificationResponse { value: Some(pb::notification_response::Value::Message("hello".into())) };
                            },
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
    
    async fn strings(&self, request: Request<pb::StringRequest>) -> Result<pb::StringResponse> {
        let pb::StringRequest { category, language } = request.into_inner();

        let category = match &*category {
            "tags" => pahkat_client::package_store::StringCategory::Tags,
            unknown => return Err(Status::failed_precondition(format!("Unknown category: {}", unknown))),
        };

        let strings = self.store.strings(category, language).await;

        // message StringResponse {
        //     message MessageMap {
        //         map<string, string> values = 1;
        //     }
        //     map<string, MessageMap> repos = 1;
        // }
        use pb::string_response::MessageMap;

        Ok(Response::new(pb::StringResponse {
            repos: strings.into_iter().map(|(k, v)| (k.to_string(), MessageMap {
                values: v.into_iter().collect()
            })).collect()
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

            while let Ok(Some(request)) = request.message().await {
                let value = match request.value {
                    Some(v) => v,
                    None => return,
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
                                actions: transaction.actions().iter().cloned().map(|x| x.into()).collect()
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
                                            value: Some(Value::DownloadError(TransactionError {
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

                        if !is_completed {
                            yield pb::TransactionResponse {
                                value: Some(Value::TransactionError(TransactionError {
                                    package_id: "".to_string(),
                                    error: "user cancelled".to_string(),
                                }))
                            };
                        }
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
                });
            }
        });

        Ok(Response::new(Box::pin(rx) as Self::ProcessTransactionStream))
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

const UPDATE_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes

pub async fn start(
    path: &Path,
    config_path: Option<&Path>,
) -> std::result::Result<(), anyhow::Error> {
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

    let mut endpoint = tokio::net::UnixListener::bind(&path).unwrap();
    let current_transaction = Arc::new(tokio::sync::Mutex::new(()));

    let incoming = endpoint.incoming().map(|x| match x {
        Ok(v) => {
            log::debug!("PEER: {:?}", &v.peer_cred());
            Ok(v)
        }
        Err(e) => Err(e),
    }); //.expect("failed to open new socket");
    let (sender, mut _rx) = broadcast::channel(5);

    let store = store(config_path).await?;
    log::debug!("Created store.");

    let rpc = Rpc {
        store: Arc::clone(&store),
        notifications: sender,
        current_transaction: Arc::clone(&current_transaction),
    };

    let sigint_listener = signal(SignalKind::interrupt())?.into_future();
    let sigterm_listener = signal(SignalKind::terminate())?.into_future();
    let sigquit_listener = signal(SignalKind::quit())?.into_future();

    // SIGUSR1 and SIGUSR2 do nothing, just swallow events.
    let _sigusr1_listener = signal(SignalKind::user_defined1())?.into_future();
    let _sigusr2_listener = signal(SignalKind::user_defined2())?.into_future();

    log::debug!("Created signal listeners for: SIGINT, SIGTERM, SIGQUIT, SIGUSR1, SIGUSR2.");

    // Create the background updater
    tokio::spawn(async move {
        let current_transaction = Arc::clone(&current_transaction);
        let store = Arc::clone(&store);

        use tokio::time::{self, Duration};
        let mut interval = time::interval(UPDATE_INTERVAL);

        loop {
            interval.tick().await;
            log::info!("Running update check…");

            // Currently installed packages:
            log::debug!("Iterating through all known packages...");
            let updates = {
                let _ = store.refresh_repos().await;
                let repos = store.repos();
                let repos = repos.read().unwrap();

                let mut updates = vec![];

                for (url, repo) in repos.iter() {
                    log::debug!("## Repo: {:?}", &url);
                    let statuses = store.all_statuses(url, pahkat_client::InstallTarget::System);

                    for (key, value) in statuses.into_iter() {
                        log::debug!(" - {:?}: {:?}", &key, &value);
                        if let Ok(pahkat_client::PackageStatus::RequiresUpdate) = value {
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

            let (_, mut stream) = transaction.process();

            // futures::pin_mut!(stream);

            if let Some(message) = stream.next().await {
                log::trace!("{:?}", message);
            }
        }
    });

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(rpc))
        .serve_with_incoming_shutdown(incoming, async move {
            tokio::select! {
                _ = sigint_listener => {
                    log::info!("SIGINT received; gracefully shutting down.");
                },
                _ = sigterm_listener => {
                    log::info!("SIGTERM received; gracefully shutting down.");
                }
                _ = sigquit_listener => {
                    log::info!("SIGQUIT received; gracefully shutting down.");
                }
            };
            ()
        })
        .await?;

    drop(endpoint);

    log::info!("Cleaning up Unix socket at path: {}", &path.display());
    std::fs::remove_file(&path)?;

    log::info!("Shutdown complete!");
    Ok(())
}
