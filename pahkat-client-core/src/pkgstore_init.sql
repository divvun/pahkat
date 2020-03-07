BEGIN;

CREATE TABLE meta (
    schema_version  INTEGER NOT NULL
);

REPLACE INTO meta(schema_version) VALUES (1);

CREATE TABLE packages (
    id              INTEGER PRIMARY KEY,
    url             TEXT NOT NULL,
    version         TEXT NOT NULL
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
    file_url       TEXT NOT NULL,

    PRIMARY KEY (package_id, file_url),
    FOREIGN KEY (package_id) REFERENCES packages(id)
);

COMMIT;