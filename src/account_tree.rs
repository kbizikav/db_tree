use crate::{
    incremental_merkle_tree::HistoricalIncrementalMerkleTree,
    indexed_merkle_tree::HistoricalIndexedMerkleTree, merkle_tree::MerkleTreeClient,
};
use anyhow::Result;
use intmax2_zkp::{
    ethereum_types::u256::U256,
    utils::trees::indexed_merkle_tree::{
        insertion::IndexedInsertionProof, leaf::IndexedMerkleLeaf, membership::MembershipProof,
        update::UpdateProof,
    },
};

type V = IndexedMerkleLeaf;
pub type HistoricalAccountTree<DB> = HistoricalIndexedMerkleTree<DB>;

impl<DB: MerkleTreeClient<V>> HistoricalAccountTree<DB> {
    pub fn new(db: DB) -> Self {
        let tree = HistoricalIncrementalMerkleTree::new(db);
        let tree = HistoricalIndexedMerkleTree(tree);
        tree
    }

    pub async fn prove_membership(&self, timestamp: u64, key: U256) -> Result<MembershipProof> {
        if let Some(index) = self.index(timestamp, key).await? {
            // inclusion proof
            return Ok(MembershipProof {
                is_included: true,
                leaf_index: index,
                leaf: self.0.get_leaf(timestamp, index).await?,
                leaf_proof: self.0.prove(timestamp, index).await?,
            });
        } else {
            // exclusion proof
            let low_index = self.low_index(timestamp, key).await?; // unwrap is safe here
            return Ok(MembershipProof {
                is_included: false,
                leaf_index: low_index,
                leaf: self.0.get_leaf(timestamp, low_index).await?,
                leaf_proof: self.0.prove(timestamp, low_index).await?,
            });
        }
    }

    pub async fn insert(&mut self, timestamp: u64, key: U256, value: u64) -> Result<()> {
        let index = self.0.len(timestamp).await? as u64;
        let low_index = self.low_index(timestamp, key).await?;
        let prev_low_leaf = self.0.get_leaf(timestamp, low_index).await?;
        let new_low_leaf = IndexedMerkleLeaf {
            next_index: index,
            next_key: key,
            ..prev_low_leaf
        };
        let leaf = IndexedMerkleLeaf {
            next_index: prev_low_leaf.next_index,
            key,
            next_key: prev_low_leaf.next_key,
            value,
        };
        self.0.update(timestamp, low_index, new_low_leaf).await?;
        self.0.push(timestamp, leaf).await?;
        Ok(())
    }

    pub async fn prove_and_insert(
        &self,
        timestamp: u64,
        key: U256,
        value: u64,
    ) -> Result<IndexedInsertionProof> {
        let index = self.0.len(timestamp).await? as u64;
        let low_index = self.low_index(timestamp, key).await?;
        let prev_low_leaf = self.0.get_leaf(timestamp, low_index).await?;
        let new_low_leaf = IndexedMerkleLeaf {
            next_index: index,
            next_key: key,
            ..prev_low_leaf
        };
        let leaf = IndexedMerkleLeaf {
            next_index: prev_low_leaf.next_index,
            key,
            next_key: prev_low_leaf.next_key,
            value,
        };
        let low_leaf_proof = self.0.prove(timestamp, low_index).await?;
        self.0.update(timestamp, low_index, new_low_leaf).await?;
        self.0.push(timestamp, leaf).await?;
        let leaf_proof = self.0.prove(timestamp, index).await?;
        Ok(IndexedInsertionProof {
            index,
            low_leaf_proof,
            leaf_proof,
            low_leaf_index: low_index,
            prev_low_leaf,
        })
    }

    pub async fn prove_and_update(
        &self,
        timestamp: u64,
        key: U256,
        new_value: u64,
    ) -> Result<UpdateProof> {
        let index = self
            .index(timestamp, key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("key not found"))?;
        let prev_leaf = self.0.get_leaf(timestamp, index).await?;
        let new_leaf = IndexedMerkleLeaf {
            value: new_value,
            ..prev_leaf
        };
        self.0.update(timestamp, index, new_leaf).await?;
        Ok(UpdateProof {
            leaf_proof: self.0.prove(timestamp, index).await?,
            leaf_index: index,
            prev_leaf,
        })
    }
}

// #[cfg(test)]
// mod tests {
//     use intmax2_zkp::utils::trees::indexed_merkle_tree::leaf::IndexedMerkleLeaf;

//     use crate::{
//         account_tree::HistoricalAccountTree,
//         node::{NodeDB as _, SqlNodeDB},
//     };

//     #[tokio::test]
//     async fn test_account_tree() -> anyhow::Result<()> {
//         let database_url = crate::setup_test();

//         let tag = 4;
//         let node_db = SqlNodeDB::<IndexedMerkleLeaf>::new(&database_url, tag).await?;
//         node_db.reset().await?;
//         // let node_db = crate::node::MockNodeDB::new();

//         let account_tree = HistoricalAccountTree::initialize(node_db).await?;

//         for i in 2..5 {
//             account_tree.insert(i.into(), i.into()).await?;
//         }
//         let old_root = account_tree.get_current_root().await?;
//         let old_leaves = account_tree.get_current_leaves().await?;
//         for i in 5..8 {
//             account_tree.insert(i.into(), i.into()).await?;
//         }
//         let leaves = account_tree.get_leaves_by_root(old_root).await?;
//         assert_eq!(leaves, old_leaves);

//         let account_id = 3;
//         let proof = account_tree
//             .prove_inclusion_by_root(old_root, account_id)
//             .await?;
//         let result = proof.verify(old_root, account_id, (account_id as u32).into());
//         assert!(result);

//         Ok(())
//     }
// }
