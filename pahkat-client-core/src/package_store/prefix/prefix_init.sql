BEGIN;

CREATE TABLE meta (
    schema_version  INTEGER NOT NULL
);

REPLACE INTO meta(schema_version) VALUES (1);

CREATE TABLE packages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    url             TEXT NOT NULL UNIQUE,
    version         TEXT NOT NULL,
    installed_on    TEXT NOT NULL,
    updated_on      TEXT NOT NULL,
    is_dependent    BOOLEAN NOT NULL DEFAULT 0,
    is_pegged       BOOLEAN NOT NULL DEFAULT 0,
);

CREATE INDEX idx_packages_url ON packages (url);

CREATE TABLE packages_dependencies (
    package_id      INTEGER NOT NULL,
    dependency_id   INTEGER NOT NULL,

    PRIMARY KEY (package_id, dependency_id),
    FOREIGN KEY (package_id) REFERENCES packages(id),
    FOREIGN KEY (dependency_id) REFERENCES packages(id)
);

CREATE TABLE packages_files (
    package_id      INTEGER NOT NULL,
    file_path       TEXT NOT NULL,

    PRIMARY KEY (package_id, file_path),
    FOREIGN KEY (package_id) REFERENCES packages(id)
);

COMMIT;