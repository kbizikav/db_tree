use thiserror::Error;

#[derive(Error, Debug)]
pub enum NodeDBError {
    #[error("Failed to connect to database: {0}")]
    ConnectionError(#[from] sqlx::Error),

    #[error("Failed to serialize/deserialize data: {0}")]
    SerializationError(#[from] bincode::Error),
}

