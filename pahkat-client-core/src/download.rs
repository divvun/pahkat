use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use reqwest::header;
use url::Url;

use crate::ext::PathExt;
use crate::package_store::DownloadEvent;

pub trait Download {
    fn download<F>(
        &self,
        tmp_dir: PathBuf,
        dir_path: &Path,
        progress: Option<F>,
    ) -> JoinHandle<Result<PathBuf, DownloadError>>
    where
        F: Fn(u64, u64) -> bool + Send + 'static;
}

pub(crate) struct DownloadManager {
    client: reqwest::Client,
    path: PathBuf,
    // max_concurrent_downloads: u8,
}

// type Stream<T> = Pin<
//     Box<dyn futures::Stream<Item = std::result::Result<T, Status>> + Send + Sync + 'static>,
// >;

impl DownloadManager {
    pub fn new(path: PathBuf, _max_concurrent_downloads: u8) -> DownloadManager {
        let client = Self::client();

        DownloadManager {
            client,
            path,
            // max_concurrent_downloads,
        }
    }

    #[inline]
    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap()
    }

    pub async fn download<P: AsRef<Path>>(
        &self,
        url: &Url,
        dest_path: P,
    ) -> Result<
        std::pin::Pin<
            Box<dyn futures::stream::Stream<Item = DownloadEvent> + Send + Sync + 'static>,
        >,
        DownloadError,
    > {
        let filename = match url.path_segments().and_then(|x| x.last()) {
            Some(v) => v,
            None => return Err(DownloadError::InvalidUrl),
        };

        let dest_path = dest_path.as_ref().to_path_buf();
        let dest_file_path = dest_path.join(filename);

        // Check destination path exists
        // if dest_path.exists() && dest_file_path.exists() {
        //     match dest_path.metadata() {
        //         Ok(v) if v.len() > 0 => {
        //             // self.handle_callback(0, 0, progress.as_ref())?;

        //             log::debug!("Download already exists at {:?}; using.", &dest_file_path);

        //             return Ok(Box::pin(async_stream::stream! {
        //                 yield DownloadEvent::Complete(dest_file_path);
        //             }));
        //         }
        //         _ => {}
        //     }
        // }

        // Create temp dirs if they don't yet exist
        if !self.path.exists() {
            fs::create_dir_all(&self.path).map_err(|e| {
                log::error!("{:?}", &e);
                DownloadError::TempDirCreateFailed(e, self.path.to_path_buf())
            })?;
        }

        // Create download dir for this file
        let cache_dir = self.path.join_sha256(url.as_str().as_bytes());
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                log::error!("{:?}", &e);
                DownloadError::TempDirCreateFailed(e, cache_dir.to_path_buf())
            })?;
        }

        let tmp_dest_path = cache_dir.join(filename);

        // let mut file = fs::OpenOptions::new()
        //     .append(true)
        //     .write(true)
        //     .open(&tmp_dest_path)
        //     .or_else(|_| )
        let file = fs::File::create(&tmp_dest_path)
            .map_err(|e| {
                log::error!("Open temp file failed: {:?}", &e);
                DownloadError::TempFileOpenFailed(e, tmp_dest_path.to_path_buf())
            })?;
        // let meta = file.metadata().map_err(|e| {
        //     log::error!("metadata error: {:?}", &e);
        //     DownloadError::MetadataFailed(e, tmp_dest_path.to_path_buf())
        // })?;

        // let mut downloaded_bytes = meta.len();
        // log::debug!("Downloaded bytes: {}", downloaded_bytes);

        let client = &self.client;
        let req = client.get(url.as_str());
        // if downloaded_bytes > 0 {
        //     req = req.header(header::RANGE, format!("bytes={}-", downloaded_bytes));
        // }

        let req = req
            .build()
            .map_err(|e| DownloadError::ReqwestError(e, url.as_str().to_string()))?;

        // Get URL headers
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let response = Self::client()
                .execute(req)
                .await
                .and_then(|x| x.error_for_status());
            tx.send(response).unwrap();
        });
        let mut res = rx
            .await
            .unwrap()
            .map_err(|e| DownloadError::ReqwestError(e, url.as_str().to_string()))?;

        // Get content length and send if exists
        let content_len = res
            .headers()
            .get(header::CONTENT_LENGTH)
            .map(|ct_len| ct_len.to_str().unwrap_or("").parse::<u64>().unwrap_or(0u64))
            .unwrap_or(0u64);
        log::debug!("Content length: {}", content_len);

        // Check if range request was accepted!
        // let is_partial = res.headers().get(header::CONTENT_RANGE).is_some();
        // log::debug!("Is partial: {}", is_partial);

        // let total_bytes = if !is_partial {
        //     std::fs::remove_file(&tmp_dest_path).map_err(|e| {
        //         log::error!("error removing temp file: {:?}", &e);
        //         DownloadError::TempFileDeleteFailed(e, tmp_dest_path.to_path_buf())
        //     })?;
        //     file = fs::File::create(&tmp_dest_path)
        //         .map_err(|e| {
        //             log::error!("{:?}", &e);
        //             DownloadError::TempFileOpenFailed(e, tmp_dest_path.to_path_buf())
        //         })?;
        //     content_len
        // } else if content_len > 0 {
        //     content_len + downloaded_bytes
        // } else {
        //     // If no content len, having downloaded bytes doesn't mean we have a known total...
        //     0
        // };

        // log::debug!("Total bytes: {}", total_bytes);

        let total_bytes = content_len;
        let mut downloaded_bytes = 0;
        let mut last_progress_event = std::time::Instant::now();

        let url = url.to_owned();
        let stream = async_stream::stream! {
            let mut file = BufWriter::new(file);
            loop {
                let chunk = res.chunk().await.map_err(|e| DownloadError::ReqwestError(e, url.as_str().to_string()));
                match chunk {
                    Ok(v) => match v {
                        None => {
                            break; // Complete
                        }
                        Some(v) => {
                            downloaded_bytes += v.len() as u64;
                            let result = file.write(&*v).map_err(|e| {
                                log::error!("error writing output: {:?}", &e);
                                DownloadError::WriteFailed(e, tmp_dest_path.to_path_buf())
                            });
                            match result {
                                Ok(_) => {
                                    // Send a progress event at most every 750ms
                                    if downloaded_bytes == total_bytes || last_progress_event.elapsed().as_millis() >= 750 {
                                        last_progress_event = std::time::Instant::now();
                                        yield DownloadEvent::Progress((downloaded_bytes, total_bytes));
                                    }
                                },
                                Err(e) => {
                                    yield DownloadEvent::Error(e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield DownloadEvent::Error(e);
                        break;
                    }
                }
            }

            match file.flush() {
                Err(e) => yield DownloadEvent::Error(DownloadError::FlushFailed(e, tmp_dest_path.to_path_buf())),
                _ => {}
            };

            drop(file);

            log::debug!("Moving {:?} to {:?}", &tmp_dest_path, &dest_path);

            // If it's done, move the file!
            let _ = fs::create_dir_all(dest_path);
            match fs::copy(&tmp_dest_path, &dest_file_path) {
                Err(e) => yield DownloadEvent::Error(DownloadError::CopyFailed(e, tmp_dest_path.to_path_buf(), dest_file_path.to_path_buf())),
                _ => {}
            };
            match fs::remove_file(&tmp_dest_path) {
                Err(e) => yield DownloadEvent::Error(DownloadError::RemoveFailed(e, tmp_dest_path.to_path_buf())),
                _ => {}
            };
            yield DownloadEvent::Complete(dest_file_path);
        };

        Ok(Box::pin(stream))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Error getting payload for package identifier")]
    Payload(#[from] crate::repo::PayloadError),

    #[error("Invalid URL")]
    InvalidUrl,

    #[error("User cancelled request")]
    UserCancelled,

    #[error("Failed to acquire file lock")]
    LockFailure,

    #[error("An internal error occurred while attempting to download: {1}")]
    ReqwestError(#[source] reqwest::Error, String),

    #[error("Failed to get metadata for file at path: {}", .1.display())]
    MetadataFailed(#[source] std::io::Error, PathBuf),

    #[error("Could not delete temporary file at path: {}", .1.display())]
    TempFileDeleteFailed(#[source] std::io::Error, PathBuf),

    #[error("Could not create temporary directory at path: {}", .1.display())]
    TempDirCreateFailed(#[source] std::io::Error, PathBuf),

    #[error("Could not open temporary file at path: {}", .1.display())]
    TempFileOpenFailed(#[source] std::io::Error, PathBuf),

    #[error("Could not flush file to disk at path: {}", .1.display())]
    FlushFailed(#[source] std::io::Error, PathBuf),

    #[error("Could not copy file from path {} to path {}", .1.display(), .2.display())]
    CopyFailed(#[source] std::io::Error, PathBuf, PathBuf),

    #[error("Could not remove file at path: {}", .1.display())]
    RemoveFailed(#[source] std::io::Error, PathBuf),

    #[error("Could not write data to file at path: {}", .1.display())]
    WriteFailed(#[source] std::io::Error, PathBuf),
}
