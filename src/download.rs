use std::path::{Path, PathBuf};
use pahkat::types::{Package, Downloadable};
use std::io::{self, BufWriter, Write};
use std::fs::File;

pub trait Download {
    fn download<F>(&self, dir_path: &Path, progress: Option<F>) -> Result<PathBuf, DownloadError>
            where F: Fn(usize, usize) -> ();
}

impl Download for Package {
    fn download<F>(&self, dir_path: &Path, progress: Option<F>) -> Result<PathBuf, DownloadError>
            where F: Fn(usize, usize) -> () {
        use reqwest::header::CONTENT_LENGTH;

        let installer = match self.installer() {
            Some(v) => v,
            None => return Err(DownloadError::NoUrl)
        };
        let url_str = installer.url();

        let url = url::Url::parse(&url_str).unwrap();
        let mut res = match reqwest::get(&url_str) {
            Ok(v) => v,
            Err(e) => return Err(DownloadError::ReqwestError(e))
        };

        if !res.status().is_success() {
            return Err(DownloadError::HttpStatusFailure(res.status().as_u16()))
        }

        let filename = &url.path_segments().unwrap().last().unwrap();
        if !dir_path.exists() {
            std::fs::create_dir_all(dir_path).unwrap();
        }
        let tmp_path = dir_path.join(&filename).to_path_buf();
        let file = File::create(&tmp_path).unwrap();
    
        let mut buf_writer = BufWriter::new(file);

        let write_res = match progress {
            Some(cb) => {
                let len = {
                    res.headers().get(CONTENT_LENGTH)
                        .map(|ct_len| ct_len.to_str().unwrap_or("").parse::<usize>().unwrap_or(0usize))
                        .unwrap_or(0usize)
                };
                res.copy_to(&mut ProgressWriter::new(buf_writer, len, cb))
            },
            None => res.copy_to(&mut buf_writer)
        };
        
        if write_res.unwrap() == 0 {
            return Err(DownloadError::EmptyFile);
        }

        Ok(tmp_path)
    }
}

#[derive(Debug)]
pub enum DownloadError {
    EmptyFile,
    InvalidUrl,
    NoUrl,
    ReqwestError(reqwest::Error),
    HttpStatusFailure(u16)
}

struct ProgressWriter<W: Write, F>
    where F: Fn(usize, usize) -> ()
{
    writer: W,
    callback: F,
    max_count: usize,
    cur_count: usize
}

impl<W: Write, F> ProgressWriter<W, F>
    where F: Fn(usize, usize) -> ()
{
    fn new(writer: W, max_count: usize, callback: F) -> ProgressWriter<W, F> {
        (callback)(0, max_count);

        ProgressWriter {
            writer: writer,
            callback: callback,
            max_count: max_count,
            cur_count: 0
        }
    }
}

impl<W: Write, F> Write for ProgressWriter<W, F>
    where F: Fn(usize, usize) -> ()
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use std::cmp;
        
        let new_count = self.cur_count + buf.len();
        self.cur_count = cmp::min(new_count, self.max_count);
        (self.callback)(self.cur_count, self.max_count);
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
