CREATE TABLE IF NOT EXISTS hash_nodes (
    timestamp_value bigint NOT NULL,
    tag int NOT NULL,
    bit_path bytea NOT NULL,
    parent_hash bytea NOT NULL,
    left_hash bytea NOT NULL,
    right_hash bytea NOT NULL,
    PRIMARY KEY (tag, bit_path, parent_hash)
);

CREATE TABLE IF NOT EXISTS leaves (
    timestamp_value bigint NOT NULL,
    tag int NOT NULL,
    position bigint NOT NULL,
    leaf_hash bytea NOT NULL,
    leaf bytea NOT NULL,
    PRIMARY KEY (tag, position, leaf_hash)
);

CREATE TABLE IF NOT EXISTS indexed_merkle_leaves (
    tag int NOT NULL,
    position bigint NOT NULL,
    leaf_hash bytea NOT NULL,
    next_index_value bigint,
    key_value NUMERIC(78),
    next_key_value NUMERIC(78),
    value_value bigint,
    PRIMARY KEY (tag, position, leaf_hash)
);

CREATE TABLE IF NOT EXISTS root_history (
    tag int NOT NULL,
    timestamp_value bigint NOT NULL,
    root_value bytea NOT NULL,
    PRIMARY KEY (tag, timestamp_value)
);