use anyhow::ensure;
use intmax2_zkp::{
    ethereum_types::u256::U256,
    utils::trees::indexed_merkle_tree::{leaf::IndexedMerkleLeaf, IndexedMerkleProof},
};

use anyhow::Result;

use crate::{
    error::HistoricalIndexedMerkleTreeError,
    incremental_merkle_tree::HistoricalIncrementalMerkleTree, merkle_tree::HashOut, node::NodeDB,
};

type V = IndexedMerkleLeaf;
type HIMTResult<T> = Result<T, HistoricalIndexedMerkleTreeError>;

#[derive(Debug, Clone)]
pub struct HistoricalIndexedMerkleTree<DB: NodeDB<V>>(HistoricalIncrementalMerkleTree<V, DB>);

impl<DB: NodeDB<V>> HistoricalIndexedMerkleTree<DB> {
    pub async fn new(node_db: DB, height: u32) -> HIMTResult<Self> {
        let mut tree = HistoricalIncrementalMerkleTree::new(node_db, height).await?;
        tree.push(IndexedMerkleLeaf::default()).await?;
        Ok(Self(tree))
    }

    pub async fn get_leaf_by_root(
        &self,
        root: HashOut<V>,
        index: u64,
    ) -> HIMTResult<IndexedMerkleLeaf> {
        let leaf = self.0.get_leaf_by_root(root, index).await?;
        Ok(leaf)
    }

    pub async fn prove_by_root(
        &self,
        root: HashOut<V>,
        index: u64,
    ) -> HIMTResult<IndexedMerkleProof> {
        let proof = self.0.prove_by_root(root, index).await?;
        Ok(proof)
    }

    pub(crate) async fn low_index(leaves: &[V], key: U256) -> HIMTResult<u64> {
        let low_leaf_candidates = leaves
            .into_iter()
            .enumerate()
            .filter(|(_, leaf)| {
                (leaf.key < key) && (key < leaf.next_key || leaf.next_key == U256::default())
            })
            .collect::<Vec<_>>();
        if low_leaf_candidates.is_empty() {
            return Err(HistoricalIndexedMerkleTreeError::KeyAlreadyExists(key));
        }
        if low_leaf_candidates.len() > 1 {
            return Err(HistoricalIndexedMerkleTreeError::TooManyCandidates);
        }
        let (low_leaf_index, _) = low_leaf_candidates[0];
        Ok(low_leaf_index as u64)
    }

    pub async fn index(leaves: &[V], key: U256) -> HIMTResult<Option<u64>> {
        let leaf_candidates = leaves
            .into_iter()
            .enumerate()
            .filter(|(_, leaf)| leaf.key == key)
            .collect::<Vec<_>>();
        if leaf_candidates.is_empty() {
            return Ok(None);
        }
        if leaf_candidates.len() > 1 {
            return Err(HistoricalIndexedMerkleTreeError::TooManyCandidates);
        }
        let (leaf_index, _) = leaf_candidates[0];
        Ok(Some(leaf_index as u64))
    }

    pub async fn key_by_root(&self, root: HashOut<V>, index: u64) -> HIMTResult<U256> {
        let key = self.0.get_leaf_by_root(root, index).await?.key;
        Ok(key)
    }

    pub async fn update(&mut self, key: U256, value: u64) -> HIMTResult<()> {
        let root = self.0.get_root()?;
        let index = self
            .index(key)
            .await?
            .ok_or_else(|| HistoricalIndexedMerkleTreeError::KeyDoesNotExist(key))?;
        let mut leaf = self.0.get_leaf_by_root(root, index).await?;
        leaf.value = value;
        self.0.update(index, leaf);
        Ok(())
    }

    pub async fn len(&self) -> HIMTResult<u32> {
        let len = self.0.len().await?;
        Ok(len)
    }
}
