use anyhow::{anyhow, ensure};
use intmax2_zkp::utils::{
    poseidon_hash_out::PoseidonHashOut,
    trees::indexed_merkle_tree::{leaf::IndexedMerkleLeaf, IndexedMerkleProof},
};

use anyhow::Result;

use crate::{
    incremental_merkle_tree::HistoricalIncrementalMerkleTree, merkle_tree::HMTResult, node::NodeDB,
};

#[derive(Debug, Clone)]
pub struct HistoricalIndexedMerkleTree<DB: NodeDB<IndexedMerkleLeaf>>(
    HistoricalIncrementalMerkleTree<IndexedMerkleLeaf, DB>,
);

impl<DB: NodeDB<IndexedMerkleLeaf>> HistoricalIndexedMerkleTree<DB> {
    pub async fn new(node_db: DB, height: u32) -> HMTResult<Self> {
        let mut tree = HistoricalIncrementalMerkleTree::new(node_db, height).await?;
        tree.push(IndexedMerkleLeaf::default());
        Self(tree)
    }

    pub fn get_root(&self) -> HMTResult<PoseidonHashOut> {
        self.0.get_root()
    }

    pub fn get_leaf(&self, index: u64) -> HMTResult<IndexedMerkleLeaf> {
        self.0.get_leaf(index)
    }

    pub fn prove(&self, index: u64) -> HMTResult<IndexedMerkleProof> {
        self.0.prove(index)
    }

    pub(crate) fn low_index(&self, key: U256) -> HMTResult<Result<u64>> {
        let low_leaf_candidates = self
            .0
            .leaves()
            .into_iter()
            .enumerate()
            .filter(|(_, leaf)| {
                (leaf.key < key) && (key < leaf.next_key || leaf.next_key == U256::default())
            })
            .collect::<Vec<_>>();
        ensure!(0 < low_leaf_candidates.len(), "key already exists");
        ensure!(
            low_leaf_candidates.len() == 1,
            "low_index: too many candidates"
        );
        let (low_leaf_index, _) = low_leaf_candidates[0];
        Ok(low_leaf_index as u64)
    }

    pub fn index(&self, key: U256) -> HMTResult<Option<u64>> {
        let leaf_candidates = self
            .0
            .leaves()
            .into_iter()
            .enumerate()
            .filter(|(_, leaf)| leaf.key == key)
            .collect::<Vec<_>>();
        if leaf_candidates.is_empty() {
            return None;
        }
        assert!(
            leaf_candidates.len() == 1,
            "find_index: too many candidates"
        );
        let (leaf_index, _) = leaf_candidates[0];
        Some(leaf_index as u64)
    }

    pub fn key(&self, index: u64) -> HMTResult<U256> {
        self.0.get_leaf(index).key
    }

    pub fn update(&mut self, key: U256, value: u64) -> HMTResult<()> {
        let index = self
            .index(key)
            .ok_or_else(|| anyhow!("Error: key doesn't exist"))?;
        let mut leaf = self.0.get_leaf(index);
        leaf.value = value;
        self.0.update(index, leaf);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
