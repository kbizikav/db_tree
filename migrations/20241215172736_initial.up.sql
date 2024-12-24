CREATE TABLE IF NOT EXISTS hash_nodes (
    tag int NOT NULL,
    time_stamp bigint NOT NULL,
    bit_path bytea NOT NULL,
    parent_hash bytea PRIMARY KEY,
    left_hash  bytea NOT NULL,
    right_hash bytea NOT NULL,
);

CREATE TABLE IF NOT EXISTS leaves (
    tag int NOT NULL,
    time_stamp bigint NOT NULL,
    position bigint NOT NULL,
    leaf_hash bytea PRIMARY KEY,
    leaf bytea NOT NULL
);

