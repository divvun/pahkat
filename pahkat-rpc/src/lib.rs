#![recursion_limit = "1024"]

pub mod client;
pub mod server;

use futures::stream::{StreamExt, TryStreamExt};
use hyper::server::conn::Http;
use log::{error, info, warn};
use pahkat_client::{
    config::RepoRecord, package_store::InstallTarget, PackageAction, PackageActionType, PackageKey,
    PackageStatus, PackageStore, PackageTransaction,
};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use task_collection::{GlobalTokio02Spawner, TaskCollection};
use tokio::io::{AsyncRead, AsyncWrite};
#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
#[cfg(windows)]
use tokio_named_pipe::{NamedPipeListener, NamedPipeStream};
use tonic::transport::server::Connected;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tower::Service;
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
        log::trace!("pb PackageAction to PackageAction: {:?}", &input);
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
    RpcStopping,
    TransactionLocked,
    TransactionUnlocked,
}

type Result<T> = std::result::Result<Response<T>, Status>;
type Stream<T> =
    Pin<Box<dyn futures::Stream<Item = std::result::Result<T, Status>> + Send + Sync + 'static>>;

struct Rpc {
    store: Arc<dyn PackageStore>,
    notifications: broadcast::Sender<Notification>,
    current_transaction: Arc<tokio::sync::Mutex<()>>,
    requires_reboot: AtomicBool,
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
        let current_transaction = Arc::clone(&self.current_transaction);
        let requires_reboot = self
            .requires_reboot
            .load(std::sync::atomic::Ordering::SeqCst);

        // log::info!("Peer: {:?}", _request.peer_cred());

        let stream = async_stream::try_stream! {
            use pb::notification_response::ValueType;
            // Do the initial checks
            if requires_reboot {
                yield pb::NotificationResponse { value: ValueType::RebootRequired as i32 };
            }

            if current_transaction.try_lock().is_err() {
                yield pb::NotificationResponse { value: ValueType::TransactionLocked as i32 };
            }

            while let response = rx.recv().await {
                match response {
                    Ok(response) => {
                        match response {
                            Notification::RebootRequired => {
                                yield pb::NotificationResponse { value: ValueType::RebootRequired as i32 };
                            }
                            Notification::RepositoriesChanged => {
                                yield pb::NotificationResponse { value: ValueType::RepositoriesChanged as i32 };
                            }
                            Notification::RpcStopping => {
                                yield pb::NotificationResponse { value: ValueType::RpcStopping as i32 };
                                break;
                            }
                            Notification::TransactionLocked => {
                                yield pb::NotificationResponse { value: ValueType::TransactionLocked as i32 };
                            }
                            Notification::TransactionUnlocked => {
                                yield pb::NotificationResponse { value: ValueType::TransactionUnlocked as i32 };
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

    async fn refresh(&self, request: Request<pb::RefreshRequest>) -> Result<pb::RefreshResponse> {
        let errors = match self.store.force_refresh_repos().await {
            Ok(_) => HashMap::new(),
            Err(e) => e.into_iter().collect(),
        };

        if errors.len() > 0 {
            log::error!("Error refreshing via RPC");
            log::error!("{:#?}", errors);
        };

        let _ = self.notifications.send(Notification::RepositoriesChanged);
        Ok(Response::new(pb::RefreshResponse {}))
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

    async fn dependency_status(
        &self,
        request: Request<pb::StatusRequest>,
    ) -> Result<pb::DependencyStatusResponse> {
        let request = request.into_inner();
        let package_id = PackageKey::try_from(&*request.package_id)
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let response = self
            .store
            .dependency_status(&package_id, Default::default());

        let response = match response {
            Ok(response) => pb::DependencyStatusResponse {
                value: Some(pb::dependency_status_response::Value::Status(
                    pb::dependency_status_response::Status {
                        statuses: response
                            .into_iter()
                            .map(|item| {
                                (
                                    item.0.to_string(),
                                    match item.1 {
                                        PackageStatus::NotInstalled => 0,
                                        PackageStatus::UpToDate => 1,
                                        PackageStatus::RequiresUpdate => 2,
                                    },
                                )
                            })
                            .collect(),
                    },
                )),
            },
            Err(e) => pb::DependencyStatusResponse {
                value: Some(pb::dependency_status_response::Value::Error(
                    pb::dependency_status_response::StatusError {
                        package_id: e.package(),
                        error: e.to_string(),
                    },
                )),
            },
        };

        Ok(Response::new(response))
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
        let is_admin = request.has_admin_flag();
        let request = request.into_inner();
        let store: Arc<dyn PackageStore> = Arc::clone(&self.store as _);
        let current_transaction = Arc::clone(&self.current_transaction);
        let notifications = self.notifications.clone();

        let (mut tx, rx) = mpsc::channel(1);
        // Get messages
        tokio::spawn(async move {
            let mut has_requested = false;
            let mut has_cancelled = false;
            let mut requires_reboot = false;

            futures::pin_mut!(request);
            let collection = TaskCollection::new(GlobalTokio02Spawner);

            'listener: loop {
                let request = request.message().await;

                let value = match request {
                    Ok(Some(v)) => match v.value {
                        Some(v) => v,
                        None => return,
                    },
                    Err(err) => {
                        log::error!("{:?}", err);
                        return;
                    }
                    Ok(None) => break 'listener,
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

                let transaction = match PackageTransaction::new(Arc::clone(&store) as _, actions) {
                    Ok(v) => v,
                    Err(e) => {
                        let response = pb::TransactionResponse {
                            value: Some(pb::transaction_response::Value::TransactionError(
                                pb::transaction_response::TransactionError {
                                    package_id: "".to_string(),
                                    error: format!("{}", e),
                                },
                            )),
                        };
                        match tx.send(Ok(response)).await {
                            Ok(_) => {}
                            Err(err) => {
                                log::error!("{:?}", err);
                            }
                        }
                        break 'listener;
                    }
                };

                let store = Arc::clone(&store);
                let current_transaction = Arc::clone(&current_transaction);

                let mut tx = tx.clone();
                let notifications = notifications.clone();

                collection.spawn(async move {
                    // If there is a running transaction, we must block on this transaction and wait
                    let tx1 = tx.clone();
                    let stream = async_stream::try_stream! {
                        let tx = tx1;
                        use pahkat_client::package_store::DownloadEvent;
                        use pb::transaction_response::*;

                        #[cfg(windows)]
                        for record in transaction.actions().iter() {
                            let id = &record.action.id;
                            if record.action.target == InstallTarget::System && !is_admin {
                                yield pb::TransactionResponse {
                                    value: Some(Value::VerificationFailed(VerificationFailed {}))
                                };
                                return;
                            }
                        }

                        log::debug!("Attempting to acquire transaction lock…");
                        let _guard = {
                            match current_transaction.try_lock() {
                                Ok(v) => v,
                                Err(_) => {
                                    log::debug!("Waiting for transaction lock…");
                                    yield pb::TransactionResponse {
                                        value: Some(Value::TransactionQueued(TransactionQueued {}))
                                    };

                                    current_transaction.lock().await
                                }
                            }
                        };
                        log::debug!("Transaction lock attained.");
                        let _ = notifications.clone().send(Notification::TransactionLocked);

                        requires_reboot = transaction.is_reboot_required();

                        yield pb::TransactionResponse {
                            value: Some(Value::TransactionStarted(TransactionStarted {
                                actions: transaction.actions().iter().cloned().map(|x| x.into()).collect(),
                                is_reboot_required: requires_reboot,
                            }))
                        };

                        for record in transaction.actions().iter() {
                            let tx = tx.clone();

                            if record.action.action != PackageActionType::Install {
                                continue;
                            }

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
                        } else {
                            let mut is_reboot_required = false;
                        }

                        log::trace!("Ending transaction stream");
                        return;
                    };

                    futures::pin_mut!(stream);

                    while let Some(value) = stream.next().await {
                        match tx.send(value).await {
                            Ok(_) => {}
                            Err(err) => {
                                log::error!("{:?}", err);
                            }
                        }
                    }

                    log::trace!("Ending outer stream loop");
                });
            }

            collection.await;
            let _ = notifications.send(Notification::TransactionUnlocked);

            if requires_reboot {
                let _ = notifications.send(Notification::RebootRequired);
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

        log::debug!("Setting repo: {:?}", &request);

        let url =
            Url::parse(&request.url).map_err(|e| Status::failed_precondition(format!("{}", e)))?;
        log::trace!("Url: {:?}", &url);
        let url = pahkat_client::types::repo::RepoUrl::new(url).map_err(|e| {
            log::debug!("Bad repo url: {:?}", e);
            Status::failed_precondition(format!("{}", e))
        })?;
        log::trace!("Repo url: {:?}", &url);

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

        let errors = match self.store.force_refresh_repos().await {
            Ok(_) => HashMap::new(),
            Err(e) => e.into_iter().collect(),
        };

        let _ = self.notifications.send(Notification::RepositoriesChanged);

        let mut config = config.read().unwrap();
        let mut repos = config.repos();

        Ok(tonic::Response::new(pb::SetRepoResponse {
            records: repos
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_owned().into()))
                .collect(),
            errors: errors
                .iter()
                .map(|(k, v)| (k.to_string(), format!("{:?}", v)))
                .collect(),
        }))
    }

    async fn get_repo_records(
        &self,
        _request: tonic::Request<pb::GetRepoRecordsRequest>,
    ) -> Result<pb::GetRepoRecordsResponse> {
        let records = {
            let config = self.store.config();
            let config = config.read().unwrap();
            config
                .repos()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_owned().into()))
                .collect()
        };

        let errors = self.store.errors();
        let errors = errors.read().unwrap();

        Ok(tonic::Response::new(pb::GetRepoRecordsResponse {
            records,
            errors: errors
                .iter()
                .map(|(k, v)| (k.to_string(), format!("{:?}", v)))
                .collect(),
        }))
    }

    async fn remove_repo(
        &self,
        request: tonic::Request<pb::RemoveRepoRequest>,
    ) -> Result<pb::RemoveRepoResponse> {
        let request = request.into_inner();

        let url =
            Url::parse(&request.url).map_err(|e| Status::failed_precondition(format!("{}", e)))?;
        let url = pahkat_client::types::repo::RepoUrl::new(url)
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let config = self.store.config();

        let is_success = {
            let mut config = config.write().unwrap();
            let mut repos = config.repos_mut();
            repos
                .remove(&url)
                .map_err(|e| Status::failed_precondition(format!("{}", e)))?
        };

        let errors = match self.store.force_refresh_repos().await {
            Ok(_) => HashMap::new(),
            Err(e) => e.into_iter().collect(),
        };

        let _ = self.notifications.send(Notification::RepositoriesChanged);

        let mut config = config.read().unwrap();
        let mut repos = config.repos();

        Ok(tonic::Response::new(pb::RemoveRepoResponse {
            records: repos
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_owned().into()))
                .collect(),
            errors: errors
                .iter()
                .map(|(k, v)| (k.to_string(), format!("{:?}", v)))
                .collect(),
        }))
    }

    async fn resolve_package_query(
        &self,
        request: Request<pb::JsonRequest>,
    ) -> Result<pb::JsonResponse> {
        log::debug!("Received resolve_package_query request: {:?}", &request);
        let json = request.into_inner().json;
        let query: pahkat_client::repo::PackageQuery = serde_json::from_str(&json)
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let results = self
            .store
            .resolve_package_query(query, &[InstallTarget::System, InstallTarget::User]);
        log::debug!("resolve_package_query results: {:?}", &results);
        Ok(tonic::Response::new(pb::JsonResponse {
            json: serde_json::to_string(&results).unwrap(),
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
        Some(v) => pahkat_client::Config::load(&v, pahkat_client::Permission::ReadWrite).0,
        None => pahkat_client::Config::load_default()?,
    };
    log::debug!("{:?}", &config);

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
        Some(v) => pahkat_client::Config::load(&v, pahkat_client::Permission::ReadWrite).0,
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

#[cfg(all(unix, not(feature = "launchd")))]
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

    use std::os::unix::fs::PermissionsExt;

    let socket = tokio::net::UnixListener::bind(&path).unwrap();
    let mut meta = std::fs::metadata(&path)?;
    let mut permissions = meta.permissions();
    permissions.set_mode(0o777);
    std::fs::set_permissions(&path, permissions)?;

    // log::trace!("UDS mode: {:o}", permissions.mode());

    Ok(socket)
}

#[cfg(all(unix, feature = "launchd"))]
#[inline(always)]
fn endpoint(path: &Path) -> std::result::Result<UnixListener, anyhow::Error> {
    use std::os::unix::io::FromRawFd;

    log::debug!("Creating launchd UNIX socket named 'pahkat'...");
    let fds = raunch::activate_socket("pahkat")?;
    let path = std::env::var("PAHKATD_UDS_PATH").unwrap();

    log::debug!("UDS path: {:?}", &path);
    log::debug!("Linking private socket path to /tmp/pahkat.sock");

    let _ = std::fs::remove_file("/tmp/pahkat.sock").unwrap_or(());
    std::fs::hard_link(path, "/tmp/pahkat.sock").unwrap();

    log::debug!("Success! Unsafely converting to a UnixListener from a raw FD");
    let std_listener = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fds[0]) };
    Ok(tokio::net::UnixListener::from_std(std_listener).unwrap())
}

fn create_background_update_service(
    store: Arc<dyn PackageStore>,
    current_transaction: Arc<tokio::sync::Mutex<()>>,
    notifications: broadcast::Sender<Notification>,
) {
    const UPDATE_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes

    tokio::spawn(async move {
        let notifications = notifications.clone();
        // let current_transaction = Arc::clone(&current_transaction);
        // let store = Arc::clone(&store);

        use tokio::time::{self, Duration};
        let mut interval = time::interval(UPDATE_INTERVAL);

        'main: loop {
            let notifications = notifications.clone();
            interval.tick().await;

            time::delay_for(Duration::from_secs(2)).await;
            let _ = store.refresh_repos().await;

            log::info!("Running self-update check…");
            match server::selfupdate::self_update().await {
                Ok(v) if v => {
                    return;
                }
                Err(e) => {
                    log::error!("Self-update check failed: {:?}", e);
                }
                _ => {}
            }

            log::info!("Running update check…");

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
                                pahkat_client::types::PackageKey {
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
            let _ = notifications.send(Notification::TransactionLocked);

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

            let _ = notifications.send(Notification::TransactionUnlocked);
            log::debug!("Completed background transaction.");
        }
    });
}

#[cfg(unix)]
pub async fn start(
    path: &Path,
    config_path: Option<&Path>,
    shutdown_rx: mpsc::UnboundedReceiver<()>,
) -> std::result::Result<(), anyhow::Error> {
    match server::setup_logger("service") {
        Ok(_) => log::debug!("Logging started."),
        Err(e) => {
            log::error!("Error setting up logging:");
            log::error!("{:?}", e);
            log::error!("Attempting env_logger...");
            env_logger::try_init()?;
        }
    }

    log::info!(
        "Starting {} {}...",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let mut endpoint = endpoint(path)?;
    log::debug!("Endpoint created successfully.");
    let store = store(config_path).await?;
    log::debug!("Created store.");

    let current_transaction = Arc::new(tokio::sync::Mutex::new(()));

    // Notifications
    let (notifications, mut notif_rx) = broadcast::channel(5);

    // Create the background updater
    create_background_update_service(
        Arc::clone(&store),
        Arc::clone(&current_transaction),
        notifications.clone(),
    );

    let rpc = Rpc {
        store: Arc::clone(&store),
        notifications: notifications.clone(),
        current_transaction: Arc::clone(&current_transaction),
        requires_reboot: AtomicBool::new(false),
    };

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(rpc))
        .serve_with_incoming_shutdown(
            endpoint.incoming().map_ok(StreamBox),
            shutdown_handler(shutdown_rx, notifications, Arc::clone(&current_transaction))?,
        )
        .await?;

    log::info!("Cleaning up Unix socket at path: {}", &path.display());
    std::fs::remove_file(&path)?;

    log::info!("Shutdown complete!");
    Ok(())
}

#[cfg(unix)]
fn shutdown_handler(
    mut shutdown_rx: mpsc::UnboundedReceiver<()>,
    mut broadcast_tx: broadcast::Sender<Notification>,
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

        let _ = broadcast_tx.send(Notification::RpcStopping);
        ()
    }))
}

#[cfg(windows)]
pub async fn start(
    path: &Path,
    config_path: Option<&Path>,
    shutdown_rx: mpsc::UnboundedReceiver<()>,
) -> std::result::Result<(), anyhow::Error> {
    log::info!(
        "Starting {} {}...",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    log::debug!("Creating security descriptor for world...");
    let mut descriptor = tokio_named_pipe::secattr::SecurityDescriptor::world()?;
    log::debug!("Creating endpoint config...");
    let mut config = tokio_named_pipe::NamedPipeConfig::default();
    log::debug!("Creating security attributes...");
    config.security_attributes =
        tokio_named_pipe::secattr::SecurityAttributes::new(&mut descriptor, false);
    log::debug!("Binding named pipe...");
    let mut endpoint = NamedPipeListener::bind(path, Some(config))?;
    // endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create().unwrap());
    let mut incoming = endpoint.incoming();

    let store = store(config_path).await?;
    log::debug!("Created store.");

    let skip_admin = store
        .config()
        .read()
        .unwrap()
        .settings()
        .skip_admin_verification();
    let current_transaction = Arc::new(tokio::sync::Mutex::new(()));

    // Notifications
    let (notifications, mut notif_rx) = broadcast::channel(5);

    // Create the background updater
    create_background_update_service(
        Arc::clone(&store),
        Arc::clone(&current_transaction),
        notifications.clone(),
    );

    let rpc = Rpc {
        store: Arc::clone(&store),
        notifications: notifications.clone(),
        current_transaction: Arc::clone(&current_transaction),
        requires_reboot: AtomicBool::new(false),
    };

    let http = Http::new().http2_only(true).clone();
    let svc = pb::pahkat_server::PahkatServer::new(rpc);

    let (tx, mut rx, mut inner_rx) = server::watch::channel().await;

    let shutdown_transaction = Arc::clone(&current_transaction);
    let shutdown = async move {
        shutdown_handler(shutdown_rx, notifications, shutdown_transaction).await;
        tx.drain().await;
    };

    tokio::spawn(shutdown);

    while let Some(Ok(conn)) =
        tokio::select! {next = incoming.next() => next, _ = inner_rx.recv() => None}
    {
        let http = http.clone();
        let svc = svc.clone();
        let mut rx = rx.clone();
        tokio::spawn(async move {
            let svc = svc.clone();

            // for the love of all that is holy don't use this handle for anything other than getting connection metadata.
            let handle = server::windows::HandleHolder(conn.as_raw_handle());
            let mut conn = http.serve_connection(
                conn,
                hyper::service::service_fn(move |mut req: hyper::Request<hyper::Body>| {
                    let mut svc = svc.clone();

                    if !skip_admin {
                        match server::windows::is_connected_user_admin(handle) {
                            Ok(true) => {
                                req.add_admin_flag();
                            }
                            Ok(false) => {}
                            Err(err) => {
                                log::error!("{:?}", err);
                            }
                        }
                    } else {
                        req.add_admin_flag();
                    }

                    svc.call(req)
                }),
            );

            rx.watch(conn, |conn| conn.graceful_shutdown()).await;
        });
    }

    log::info!("Shutdown complete!");
    Ok(())
}

#[cfg(windows)]
fn shutdown_handler(
    mut shutdown_rx: mpsc::UnboundedReceiver<()>,
    mut broadcast_tx: broadcast::Sender<Notification>,
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

        let _ = broadcast_tx.send(Notification::RpcStopping);
        ()
    }
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

trait AdminCheck {
    const FLAG: &'static str = "is-admin-path";

    fn add_admin_flag(&mut self);

    fn has_admin_flag(&self) -> bool;
}

impl<T> AdminCheck for hyper::Request<T> {
    fn add_admin_flag(&mut self) {
        self.headers_mut().insert(
            <Self as AdminCheck>::FLAG,
            hyper::header::HeaderValue::from_static("true"),
        );
    }

    fn has_admin_flag(&self) -> bool {
        self.headers().contains_key(<Self as AdminCheck>::FLAG)
    }
}

impl<T> AdminCheck for Request<T> {
    fn add_admin_flag(&mut self) {
        self.metadata_mut().insert(
            <Self as AdminCheck>::FLAG,
            tonic::metadata::MetadataValue::from_static("true"),
        );
    }

    fn has_admin_flag(&self) -> bool {
        self.metadata().contains_key(<Self as AdminCheck>::FLAG)
    }
}
