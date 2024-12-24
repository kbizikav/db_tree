CREATE TABLE IF NOT EXISTS current_node_hashes (
    tag int NOT NULL,
    bit_path bytea PRIMARY KEY,
    hash_value bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS hash_nodes (
    tag int NOT NULL,
    parent_hash bytea PRIMARY KEY,
    left_hash  bytea NOT NULL,
    right_hash bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS current_leaf_hashes (
    tag int NOT NULL,
    position bigint PRIMARY KEY,
    leaf_hash bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS leaves (
    tag int NOT NULL,
    leaf_hash bytea PRIMARY KEY,
    leaf bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS root_history (
    tag int NOT NULL,
    i int PRIMARY KEY,
    root bytea NOT NULL
);