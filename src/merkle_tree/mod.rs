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
    use crate::{merkle_tree::mock_merkle_tree::MockMerkleTree, setup_test};

    use super::sql_merkle_tree::SqlMerkleTree;

    type V = u32;

    #[tokio::test]
    async fn test_merkle_tree() -> anyhow::Result<()> {
        let database_url = setup_test();

        let height = 10;
        let tree = MockMerkleTree::<V>::new(height);

        let timestamp = 0;
        for i in 0..5 {
            tree.update_leaf(timestamp, i, i as u32).await?;
        }
        let timestamp = 2;
        for i in 5..10 {
            tree.update_leaf(timestamp, i, i as u32).await?;
        }
        tree.update_leaf(timestamp, 3, 9).await?;

        let leaves0_m = tree.get_leaves_at_timestamp(0).await?;
        let leaves2_m = tree.get_leaves_at_timestamp(2).await?;
        let root0_m = tree.get_root(0).await?;
        let root2_m = tree.get_root(2).await?;

        let timestamp = 0;
        let tree = SqlMerkleTree::<V>::new(&database_url, 0, height);
        tree.reset().await?;

        for i in 0..5 {
            tree.update_leaf(timestamp, i, i as u32).await?;
        }
        let timestamp = 2;
        for i in 5..10 {
            tree.update_leaf(timestamp, i, i as u32).await?;
        }
        tree.update_leaf(timestamp, 3, 9).await?;

        let leaves0 = tree.get_leaves_at_timestamp(0).await?;
        let leaves2 = tree.get_leaves_at_timestamp(2).await?;
        let root0 = tree.get_root(0).await?;
        let root2 = tree.get_root(2).await?;

        assert_eq!(leaves0, leaves0_m);
        assert_eq!(leaves2, leaves2_m);
        assert_eq!(root0_m, root0);
        assert_eq!(root2, root2_m);

        Ok(())
    }

    #[tokio::test]
    async fn test_merkle_tree_speed() -> anyhow::Result<()> {
        let height = 32;
        let n = 1 << 12;

        let database_url = setup_test();
        let tree = SqlMerkleTree::<V>::new(&database_url, 0, height);
        tree.reset().await?;

        let timestamp = 0;
        let time = std::time::Instant::now();
        for i in 0..n {
            tree.update_leaf(timestamp, i, i as u32).await?;
        }

        println!(
            "SqlMerkleTree: {} leaves, {} height, {} seconds",
            n,
            height,
            time.elapsed().as_secs_f64()
        );

        Ok(())
    }
}
