CREATE TABLE user_access (
  id BLOB(16) NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),
  user_id BLOB(16) NOT NULL,
  timestamp DATETIME NOT NULL,
  FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE INDEX idx_user_access_timestamp ON user_access (timestamp);
