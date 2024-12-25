use async_trait::async_trait;
use error::DBClientError;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use serde::{de::DeserializeOwned, Serialize};

use crate::utils::bit_path::BitPath;

pub mod error;

pub type Hasher<V> = <V as Leafable>::LeafableHasher;
pub type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;
type Result<T> = std::result::Result<T, DBClientError>;

#[derive(Debug, Clone)]
pub struct NodeHashes<V: Leafable> {
    pub parent_hash: HashOut<V>,
    pub left_hash: HashOut<V>,
    pub right_hash: HashOut<V>,
}

#[async_trait(?Send)]
pub trait NodeDB<V: Leafable + Serialize + DeserializeOwned>: std::fmt::Debug + Clone {
    async fn save_node(
        &self,
        timestamp: u64,
        bit_path: BitPath,
        parent_hash: HashOut<V>,
        left_hash: HashOut<V>,
        right_hash: HashOut<V>,
    ) -> Result<()>;

    async fn get_node_at_timestamp(
        &self,
        timestamp: u64,
        bit_path: BitPath,
    ) -> Result<NodeHashes<V>>;

    async fn save_leaf(&self, timestamp: u64, position: u64, leaf: V) -> Result<()>;

    async fn get_leaf_at_timestamp(&self, timestamp: u64, position: u64) -> Result<V>;

    async fn get_leaves_at_timestamp(&self, timestamp: u64) -> Result<Vec<(u64, V)>>;

    async fn get_num_leaves_at_timestamp(&self, timestamp: u64) -> Result<usize>;
}
