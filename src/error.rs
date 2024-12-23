use thiserror::Error;

#[derive(Error, Debug)]
pub enum NodeDBError {
    #[error("Failed to connect to database: {0}")]
    ConnectionError(#[from] sqlx::Error),

    #[error("Failed to serialize/deserialize data: {0}")]
    SerializationError(#[from] bincode::Error),
}

#[derive(Error, Debug)]
pub enum HistoricalMerkleTreeError {
    #[error("Node DB Error: {0}")]
    NodeDBError(#[from] NodeDBError),

    #[error("Invalid path length: {0}")]
    WrongPathLength(u32),

    #[error("Node not found for parent hash: {0}")]
    NodeNotFoundError(String),

    #[error("Leaf hash mismatch: expected {expected}, got {got}")]
    LeafHashMismatch { expected: String, got: String },
}
