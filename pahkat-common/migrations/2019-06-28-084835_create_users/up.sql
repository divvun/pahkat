CREATE TABLE users (
  id BLOB(16) NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),
  username TEXT NOT NULL UNIQUE,
  token BLOB(16) NOT NULL
);

CREATE INDEX idx_users_username ON users (username);
