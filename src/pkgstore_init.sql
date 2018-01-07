BEGIN;

CREATE TABLE meta (
    schema_version  INT NOT NULL
);

REPLACE INTO meta(schema_version) VALUES (1);

CREATE TABLE packages (
    id              TEXT PRIMARY KEY,
    version         TEXT NOT NULL
);

CREATE TABLE packages_dependencies (
    package_id      TEXT NOT NULL,
    dependency_id   TEXT NOT NULL,
    PRIMARY KEY (package_id, dependency_id),
    FOREIGN KEY (package_id) REFERENCES packages(id)
);

CREATE TABLE packages_files (
    package_id      TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    PRIMARY KEY (package_id, file_path),
    FOREIGN KEY (package_id) REFERENCES packages(id)
);

COMMIT;