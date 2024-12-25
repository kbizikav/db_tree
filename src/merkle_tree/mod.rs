use async_trait::async_trait;
use error::MerkleTreeError;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use serde::{de::DeserializeOwned, Serialize};

use crate::utils::bit_path::BitPath;

pub mod error;
pub mod mock_db_client;

pub type Hasher<V> = <V as Leafable>::LeafableHasher;
pub type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;
pub type MTResult<T> = std::result::Result<T, MerkleTreeError>;

#[async_trait(?Send)]
pub trait DBClient<V: Leafable + Serialize + DeserializeOwned>: std::fmt::Debug + Clone {
    async fn save_node(&self, timestamp: u64, bit_path: BitPath, hash: HashOut<V>) -> MTResult<()>;
    async fn get_node_at_timestamp(
        &self,
        timestamp: u64,
        bit_path: BitPath,
    ) -> MTResult<HashOut<V>>;
    async fn save_leaf(&self, timestamp: u64, position: u64, leaf: V) -> MTResult<()>;
    async fn get_leaf_at_timestamp(&self, timestamp: u64, position: u64) -> MTResult<V>;
    async fn get_leaves_at_timestamp(&self, timestamp: u64) -> MTResult<Vec<(u64, V)>>;
    async fn get_num_leaves_at_timestamp(&self, timestamp: u64) -> MTResult<usize>;
}
