use intmax2_zkp::{
    common::trees::account_tree::AccountMerkleProof, constants::ACCOUNT_TREE_HEIGHT,
    ethereum_types::u256::U256, utils::trees::indexed_merkle_tree::leaf::IndexedMerkleLeaf,
};

use crate::{
    indexed_merkle_tree::{HIMTResult, HistoricalIndexedMerkleTree},
    merkle_tree::HashOut,
    node::NodeDB,
};

type V = IndexedMerkleLeaf;
pub type HistoricalAccountTree<DB> = HistoricalIndexedMerkleTree<DB>;

impl<DB: NodeDB<V>> HistoricalAccountTree<DB> {
    pub async fn initialize(node_db: DB) -> HIMTResult<Self> {
        let mut tree =
            HistoricalIndexedMerkleTree::new(node_db, ACCOUNT_TREE_HEIGHT as u32).await?;
        tree.insert(U256::dummy_pubkey(), 0).await?; // add default account
        Ok(tree)
    }

    pub async fn prove_inclusion_by_root(
        &self,
        root: HashOut<V>,
        account_id: u64,
    ) -> HIMTResult<AccountMerkleProof> {
        let leaf = self.get_leaf_by_root(root, account_id).await?;
        let merkle_proof = self.prove_by_root(root, account_id).await?;
        Ok(AccountMerkleProof { merkle_proof, leaf })
    }
}
