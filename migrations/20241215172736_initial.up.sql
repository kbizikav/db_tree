CREATE TABLE IF NOT EXISTS hash_nodes (
    timestamp_value bigint NOT NULL,
    tag int NOT NULL,
    bit_path bytea NOT NULL,
    hash_value bytea NOT NULL,
    PRIMARY KEY (timestamp_value, tag, bit_path)
);

CREATE TABLE IF NOT EXISTS leaves (
    timestamp_value bigint NOT NULL,
    tag int NOT NULL,
    position bigint NOT NULL,
    leaf_hash bytea NOT NULL,
    leaf bytea NOT NULL,
    PRIMARY KEY (timestamp_value, tag, position)
);

CREATE TABLE IF NOT EXISTS leaves_len (
    timestamp_value bigint NOT NULL,
    tag int NOT NULL,
    len int NOT NULL,
    PRIMARY KEY (timestamp_value, tag)
);

CREATE TABLE IF NOT EXISTS indexed_merkle_leaves (
    tag int NOT NULL,
    position bigint NOT NULL,
    leaf_hash bytea NOT NULL,
    next_index_value bigint,
    key_value NUMERIC(78),
    next_key_value NUMERIC(78),
    value_value bigint,
    PRIMARY KEY (timestamp_value, tag, position)
);

CREATE TABLE IF NOT EXISTS root_history (
    timestamp_value bigint NOT NULL,
    tag int NOT NULL,
    root_value bytea NOT NULL,
    PRIMARY KEY (tag, timestamp_value)
);