CREATE TABLE IF NOT EXISTS current_node_hashes (
    tag int NOT NULL,
    bit_path bytea NOT NULL,
    hash_value bytea NOT NULL,
    PRIMARY KEY (tag, bit_path)
);

CREATE TABLE IF NOT EXISTS hash_nodes (
    tag int NOT NULL,
    parent_hash bytea NOT NULL,
    left_hash  bytea NOT NULL,
    right_hash bytea NOT NULL,
    PRIMARY KEY (tag, parent_hash)
);

CREATE TABLE IF NOT EXISTS current_leaf_hashes (
    tag int NOT NULL,
    position bigint NOT NULL,
    leaf_hash bytea NOT NULL,
    PRIMARY KEY (tag, position)
);

CREATE TABLE IF NOT EXISTS leaves (
    tag int NOT NULL,
    leaf_hash bytea NOT NULL,
    leaf bytea NOT NULL,
    PRIMARY KEY (tag, leaf_hash)
);

CREATE TABLE IF NOT EXISTS root_history (
    tag int NOT NULL,
    i int NOT NULL,
    root bytea NOT NULL,
    PRIMARY KEY (tag, i)
);