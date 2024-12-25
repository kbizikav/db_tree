use async_trait::async_trait;
use error::MerkleTreeError;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use serde::{de::DeserializeOwned, Serialize};

use crate::utils::bit_path::BitPath;

pub mod error;
pub mod mock_merkle_tree;
pub mod sql_merkle_tree;

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

#[cfg(test)]
mod tests {
    use core::time;

    use sqlx::types::uuid::timestamp;

    use crate::{merkle_tree::mock_merkle_tree::MockMerkleTree, setup_test};

    use super::sql_merkle_tree::SqlMerkleTree;

    type V = u32;

    #[tokio::test]
    async fn test_merkle_tree() -> anyhow::Result<()> {
        let database_url = setup_test();

        let height = 10;
        let tree = MockMerkleTree::<V>::new(height);

        // let timestamp = 0;
        // for i in 0..5 {
        //     tree.update_leaf(timestamp, i, i as u32).await?;
        // }
        // let timestamp = 2;
        // for i in 5..10 {
        //     tree.update_leaf(timestamp, i, i as u32).await?;
        // }
        // let leaves0_m = tree.get_leaves_at_timestamp(0).await?;
        // let leaves2_m = tree.get_leaves_at_timestamp(2).await?;
        // let root_m = tree.get_root(2).await?;

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        let timestamp = 0;

        let tree = SqlMerkleTree::<V>::new(0, height);
        let mut tx = pool.begin().await?;
        tree.reset(&mut tx).await?;
        tx.commit().await?;

        let mut tx = pool.begin().await?;
        tree.update_leaf(&mut tx, timestamp, 0, 0 as u32).await?;
        // let mut tx = pool.begin().await?;
        // let mut tx = pool.begin().await?;
        // for i in 0..1 {
        //     tree.update_leaf(&mut tx, timestamp, i, i as u32).await?;
        // }
        // tx.commit().await?;
        // let timestamp = 2;

        // let mut tx = pool.begin().await?;
        // for i in 5..10 {
        //     tree.update_leaf(&mut tx, timestamp, i, i as u32).await?;
        // }
        // tx.commit().await?;

        // let mut tx = pool.begin().await?;
        // let leaves0 = tree.get_leaves_at_timestamp(&mut tx, 0).await?;
        // let leaves2 = tree.get_leaves_at_timestamp(&mut tx, 2).await?;
        // let root = tree.get_root(&mut tx, 2).await?;
        // tx.commit().await?;

        // assert_eq!(leaves0, leaves0_m);
        // assert_eq!(leaves2, leaves2_m);
        // assert_eq!(root, root_m);

        Ok(())
    }
}
