CREATE TABLE IF NOT EXISTS hash_nodes (
    parent_hash bytea PRIMARY KEY,
    left_hash  bytea NOT NULL,
    right_hash bytea NOT NULL
);

