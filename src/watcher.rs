extern crate glob;

use std::io;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use glob::glob;

#[derive(Debug)]
pub enum WatcherEvent {
    Created(String),
    Deleted(String),
    Modified(String)
}

#[derive(Debug)]
pub enum WatcherError {
    IllegalPath(String),
    IoError(io::Error),
    PatternError(glob::PatternError)
}

impl From<io::Error> for WatcherError {
    fn from(error: io::Error) -> Self {
        WatcherError::IoError(error)
    }
}

impl From<glob::PatternError> for WatcherError {
    fn from(error: glob::PatternError) -> Self {
        WatcherError::PatternError(error)
    }
}

#[derive(Debug)]
pub struct Watcher {
    glob_expression: String,
    cache: HashMap<String, SystemTime>
}

impl Watcher {
    pub fn new(path: &Path) -> Result<Self, WatcherError> {
        let glob_expression = format!("{}/**/index.json", path.display()); 
        if !Path::new(path).is_dir() {
            return Err(WatcherError::IllegalPath(path.display().to_string()));
        }
        Ok(Watcher {
            glob_expression,
            cache: HashMap::<String, SystemTime>::new()
        })
    }

    pub fn update(&mut self) -> Result<Vec<WatcherEvent>, WatcherError> {
        let mut result = Vec::<WatcherEvent>::new();

        let matches: Vec<String> = glob(&self.glob_expression)?
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(path) => Some(path.display().to_string()),
                _ => None
            })
            .collect();

        let mut removed_indexes = Vec::<String>::new();        
        for cached_index in self.cache.keys() {
            let count = matches
                .iter()
                .filter(|index| index.as_str() == cached_index.as_str())
                .count();
            if count == 0 {
                removed_indexes.push(cached_index.to_string());
            }
        }

        for index in removed_indexes {
            self.cache.remove(&index);
            // println!("Index {} has been deleted", &index);
            result.push(WatcherEvent::Deleted(index.clone()));
        }

        for index in matches {
            let metadata = fs::metadata(Path::new(&index))?;
            let modified = metadata.modified()?;

            match self.cache.get_mut(&index) {
                Some(cached_modified) => {
                    if modified != *cached_modified {
                        *cached_modified = modified;
                        // println!("Index {} has been modified", index.as_str());
                        result.push(WatcherEvent::Modified(index.clone()));
                    }
                }
                None => {
                    self.cache.insert(index.to_string(), modified);
                    // println!("Index {} has been created", index.as_str());
                    result.push(WatcherEvent::Created(index.clone()));
                }
            }
        }

        Ok(result)
    }
}
