use std::path::{Path, PathBuf};

use sha2::digest::Digest;
use sha2::Sha256;

pub(crate) trait PathExt {
    fn join_sha256(&self, bytes: &[u8]) -> PathBuf;
}

impl PathExt for Path {
    fn join_sha256(&self, bytes: &[u8]) -> PathBuf {
        let mut sha = Sha256::new();
        sha.update(bytes);
        let hash_id = format!("{:x}", sha.finalize());
        let part1 = &hash_id[0..2];
        let part2 = &hash_id[2..4];
        let part3 = &hash_id[4..];
        self.join(part1).join(part2).join(part3)
    }
}
