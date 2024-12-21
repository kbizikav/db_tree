CREATE TABLE IF NOT EXISTS hash_nodes (
    parent_hash bytea PRIMARY KEY,
    left_hash  bytea NOT NULL,
    right_hash bytea NOT NULL
);

CREATE TABLE IF NOT EXISTS current_leaves (
    i int PRIMARY KEY,
    leaf bytea NOT NULL
)

CREATE TABLE IF NOT EXISTS root_history (
    i int PRIMARY KEY,
    root bytea NOT NULL
)