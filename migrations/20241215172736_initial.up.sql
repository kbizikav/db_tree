CREATE TABLE IF NOT EXISTS hash_nodes (
    parent_hash bytea PRIMARY KEY,
    left_hash  bytea NOT NULL,
    right_hash bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS current_leaf_hashes (
    position bigint PRIMARY KEY,
    leaf_hash bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS current_leaves (
    position bigint PRIMARY KEY,
    leaf bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS root_history (
    i int PRIMARY KEY,
    root bytea NOT NULL
);