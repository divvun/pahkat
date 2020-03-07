use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use fd_lock::FdLock;
use reqwest::header;
use url::Url;

use crate::ext::PathExt;

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
    max_concurrent_downloads: u8,
}

impl DownloadManager {
    pub fn new(path: PathBuf, max_concurrent_downloads: u8) -> DownloadManager {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap();

        DownloadManager {
            client,
            path,
            max_concurrent_downloads,
        }
    }

    #[inline(always)]
    fn handle_callback<F>(
        &self,
        cur: u64,
        max: u64,
        progress: Option<&F>,
    ) -> Result<(), DownloadError>
    where
        F: Fn(u64, u64) -> bool + Send + 'static,
    {
        if let Some(cb) = progress {
            let should_continue = cb(cur, max);

            if !should_continue {
                return Err(DownloadError::UserCancelled);
            }
        }

        Ok(())
    }

    pub async fn download<F, P: AsRef<Path>>(
        &self,
        url: &Url,
        dest_path: P,
        progress: Option<F>,
    ) -> Result<PathBuf, DownloadError>
    where
        F: Fn(u64, u64) -> bool + Send + 'static,
    {
        let filename = match url.path_segments().and_then(|x| x.last()) {
            Some(v) => v,
            None => return Err(DownloadError::InvalidUrl),
        };

        let dest_path = dest_path.as_ref();
        let dest_file_path = dest_path.join(filename);

        // Check destination path exists
        if dest_path.exists() && dest_file_path.exists() {
            self.handle_callback(0, 0, progress.as_ref())?;

            log::debug!("Download already exists at {:?}; using.", &dest_file_path);

            return Ok(dest_file_path);
        }

        // Create temp dirs if they don't yet exist
        if !self.path.exists() {
            fs::create_dir_all(&self.path).map_err(|e| DownloadError::IoError(e))?;
        }

        // Create download dir for this file
        let cache_dir = self.path.join_sha256(url.as_str().as_bytes());
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| DownloadError::IoError(e))?;
        }

        let tmp_dest_path = cache_dir.join(filename);
        let mut req = self.client.get(url.as_str());

        let mut fdlock = {
            let fd = fs::OpenOptions::new()
                .append(true)
                .open(&tmp_dest_path)
                .or_else(|_| fs::File::create(&tmp_dest_path))
                .map_err(|e| DownloadError::IoError(e))?;
            FdLock::new(fd)
        };

        // Lock temporary destination file for writing
        log::debug!("Locking {}", tmp_dest_path.display());

        #[cfg(not(windows))]
        let mut file = fdlock.lock().map_err(|_| DownloadError::LockFailure)?;
        #[cfg(windows)]
        let mut file = fdlock.try_lock().map_err(|_| DownloadError::LockFailure)?;

        log::debug!("Got lock on {}", tmp_dest_path.display());
        let meta = file.metadata().map_err(|e| DownloadError::IoError(e))?;

        let mut downloaded_bytes = meta.len();
        log::debug!("Downloaded bytes: {}", downloaded_bytes);

        if downloaded_bytes > 0 {
            req = req.header(header::RANGE, format!("bytes={}-", downloaded_bytes));
        }

        let req = req.build().map_err(DownloadError::ReqwestError)?;

        // Get URL headers
        let mut res = self
            .client
            .execute(req)
            .await
            .map_err(DownloadError::ReqwestError)?;

        // Get content length and send if exists
        let content_len = res
            .headers()
            .get(header::CONTENT_LENGTH)
            .map(|ct_len| ct_len.to_str().unwrap_or("").parse::<u64>().unwrap_or(0u64))
            .unwrap_or(0u64);
        log::debug!("Content length: {}", content_len);

        // Check if range request was accepted!
        let is_partial = res.headers().get(header::CONTENT_RANGE).is_some();
        log::debug!("Is partial: {}", is_partial);

        let total_bytes = if !is_partial {
            file.set_len(0).map_err(|e| DownloadError::IoError(e))?;
            content_len
        } else if content_len > 0 {
            content_len + downloaded_bytes
        } else {
            // If no content len, having downloaded bytes doesn't mean we have a known total...
            0
        };

        log::debug!("Total bytes: {}", total_bytes);

        {
            let mut file = BufWriter::new(&mut *file);

            // Do the download
            while let Some(chunk) = res.chunk().await.map_err(DownloadError::ReqwestError)? {
                downloaded_bytes += chunk.len() as u64;
                file.write(&*chunk).map_err(DownloadError::IoError)?;
                self.handle_callback(downloaded_bytes, total_bytes, progress.as_ref())?;
            }
        }

        log::debug!("Moving {:?} to {:?}", &tmp_dest_path, &dest_path);

        // If it's done, move the file!
        let _ = fs::create_dir_all(dest_path);
        fs::rename(tmp_dest_path, &dest_file_path).map_err(DownloadError::IoError)?;

        Ok(dest_file_path)
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

    #[error("File IO error")]
    IoError(#[from] std::io::Error),

    #[error("Error downloading file")]
    ReqwestError(#[from] reqwest::Error),
    // PersistError(tempfile::PersistError),
    // HttpStatusFailure(u16),
}
