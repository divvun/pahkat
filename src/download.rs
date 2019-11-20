use pahkat_types::{Downloadable, Package};

use std::error::Error;
use std::io::ErrorKind;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

pub trait Download {
    fn download<F>(
        &self,
        tmp_path: PathBuf,
        dir_path: &Path,
        progress: Option<F>,
    ) -> JoinHandle<Result<PathBuf, DownloadError>>
    where
        F: Fn(u64, u64) -> bool + Send + 'static;
}

const USER_CANCELLED_STR: &str = "User cancelled";

impl Download for Package {
    fn download<F>(
        &self,
        tmp_path: PathBuf,
        dir_path: &Path,
        progress: Option<F>,
    ) -> JoinHandle<Result<PathBuf, DownloadError>>
    where
        F: Fn(u64, u64) -> bool + Send + 'static,
    {
        use reqwest::header::CONTENT_LENGTH;

        let dir_path = dir_path.to_owned();
        let installer = self.installer().cloned();

        let handle = std::thread::spawn(move || {
            let installer = match installer {
                Some(v) => v,
                None => return Err(DownloadError::NoUrl),
            };

            let url_str = installer.url();
            let url = url::Url::parse(&url_str).map_err(|_| DownloadError::InvalidUrl)?;
            let filename = Path::new(url.path_segments().unwrap().last().unwrap()).to_path_buf();

            if filename.exists() {
                if let Some(cb) = progress {
                    cb(0, 0);
                }

                return Ok(filename);
            }

            let mut res = match reqwest::get(&url_str) {
                Ok(v) => v,
                Err(e) => return Err(DownloadError::ReqwestError(e)),
            };

            if !res.status().is_success() {
                return Err(DownloadError::HttpStatusFailure(res.status().as_u16()));
            }

            if !tmp_path.exists() {
                std::fs::create_dir_all(&tmp_path).unwrap();
            }

            if !dir_path.exists() {
                std::fs::create_dir_all(&dir_path).unwrap();
            }

            let file = tempfile::NamedTempFile::new_in(tmp_path).expect("temp files");
            let mut buf_writer = BufWriter::new(file.reopen().unwrap());

            let write_res = match progress {
                Some(cb) => {
                    let len = {
                        res.headers()
                            .get(CONTENT_LENGTH)
                            .map(|ct_len| {
                                ct_len.to_str().unwrap_or("").parse::<u64>().unwrap_or(0u64)
                            })
                            .unwrap_or(0u64)
                    };

                    res.copy_to(&mut ProgressWriter::new(buf_writer, len, cb))
                }
                None => res.copy_to(&mut buf_writer),
            };

            match write_res {
                Ok(v) if v == 0 => {
                    return Err(DownloadError::EmptyFile);
                }
                Err(e) => match e.get_ref().and_then(|e| e.downcast_ref::<std::io::Error>()) {
                    Some(e)
                        if e.kind() == ErrorKind::Other && e.description() == USER_CANCELLED_STR =>
                    {
                        return Err(DownloadError::UserCancelled)
                    }
                    _ => {
                        log::debug!("{:?}", e);
                        return Err(DownloadError::ReqwestError(e));
                    }
                },
                _ => {}
            }

            let out_path = (&dir_path).join(&filename).to_path_buf();
            file.persist(&out_path)
                .map_err(|e| DownloadError::PersistError(e))?;

            Ok(out_path)
        });

        handle
    }
}

#[derive(Debug)]
pub enum DownloadError {
    EmptyFile,
    InvalidUrl,
    NoUrl,
    UserCancelled,
    ReqwestError(reqwest::Error),
    PersistError(tempfile::PersistError),
    HttpStatusFailure(u16),
}

impl std::error::Error for DownloadError {}
impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

struct ProgressWriter<W: Write, F>
where
    F: Fn(u64, u64) -> bool,
{
    writer: W,
    callback: F,
    max_count: u64,
    cur_count: u64,
}

impl<W: Write, F> ProgressWriter<W, F>
where
    F: Fn(u64, u64) -> bool,
{
    fn new(writer: W, max_count: u64, callback: F) -> ProgressWriter<W, F> {
        (callback)(0, max_count);

        ProgressWriter {
            writer,
            callback,
            max_count,
            cur_count: 0,
        }
    }
}

impl<W: Write, F> Write for ProgressWriter<W, F>
where
    F: Fn(u64, u64) -> bool,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use std::cmp;

        let new_count = self.cur_count + buf.len() as u64;
        self.cur_count = cmp::min(new_count, self.max_count);
        let is_cancelled = !(self.callback)(self.cur_count, self.max_count);

        if is_cancelled {
            return Err(std::io::Error::new(ErrorKind::Other, USER_CANCELLED_STR));
        }

        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
