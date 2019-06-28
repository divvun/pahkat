CREATE TABLE downloads (
  id BLOB(16) NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),
  package_id TEXT NOT NULL,
  package_version TEXT NOT NULL,
  timestamp DATETIME NOT NULL
);

CREATE INDEX idx_downloads_package_id ON downloads (package_id);
CREATE INDEX idx_downloads_package_package_version ON downloads (package_version);
CREATE INDEX idx_downloads_timestamp ON downloads (timestamp);
