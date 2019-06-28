CREATE TABLE users (
  id BLOB(16) NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),
  name TEXT NOT NULL UNIQUE,
  token BLOB(16) NOT NULL
);

CREATE INDEX idx_name ON users (name);
