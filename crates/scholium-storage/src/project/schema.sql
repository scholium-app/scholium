PRAGMA application_id = 1396919378;
PRAGMA user_version = 1;
CREATE TABLE resources (
    id BLOB PRIMARY KEY CHECK(length(id)=16),
    path TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE actions (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT CHECK(sequence>0),
    version INTEGER NOT NULL CHECK(version=1),
    payload BLOB NOT NULL,
    hash BLOB NOT NULL CHECK(length(hash)=32)
) STRICT;
CREATE TABLE blobs (
    hash BLOB PRIMARY KEY CHECK(length(hash)=32),
    bytes BLOB NOT NULL
) STRICT;
CREATE TABLE drafts (
    resource BLOB PRIMARY KEY REFERENCES resources(id),
    version INTEGER NOT NULL CHECK(version=1),
    base_hash BLOB CHECK(base_hash IS NULL OR length(base_hash)=32),
    bytes BLOB NOT NULL,
    hash BLOB NOT NULL CHECK(length(hash)=32)
) STRICT;
CREATE TABLE manifests (
    resource BLOB PRIMARY KEY REFERENCES resources(id),
    version INTEGER NOT NULL CHECK(version=1),
    revision INTEGER NOT NULL CHECK(revision>0),
    old_hash BLOB CHECK(old_hash IS NULL OR length(old_hash)=32),
    new_hash BLOB NOT NULL REFERENCES blobs(hash),
    pending INTEGER NOT NULL CHECK(pending IN (0,1))
) STRICT;
CREATE TABLE snapshots (
    sequence INTEGER PRIMARY KEY REFERENCES actions(sequence),
    version INTEGER NOT NULL CHECK(version=1),
    bytes BLOB NOT NULL,
    hash BLOB NOT NULL CHECK(length(hash)=32)
) STRICT;
CREATE TRIGGER actions_no_update BEFORE UPDATE ON actions BEGIN
    SELECT RAISE(ABORT, 'actions are append-only'); END;
CREATE TRIGGER actions_no_delete BEFORE DELETE ON actions BEGIN
    SELECT RAISE(ABORT, 'actions are append-only'); END;
CREATE TRIGGER blobs_no_update BEFORE UPDATE ON blobs BEGIN
    SELECT RAISE(ABORT, 'blobs are immutable'); END;
CREATE TRIGGER blobs_no_delete BEFORE DELETE ON blobs BEGIN
    SELECT RAISE(ABORT, 'blobs are immutable'); END;
CREATE TRIGGER snapshots_no_update BEFORE UPDATE ON snapshots BEGIN
    SELECT RAISE(ABORT, 'snapshots are immutable'); END;
CREATE TRIGGER snapshots_no_delete BEFORE DELETE ON snapshots BEGIN
    SELECT RAISE(ABORT, 'snapshots are immutable'); END;
