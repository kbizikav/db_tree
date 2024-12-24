use anyhow::ensure;
use intmax2_zkp::{
    common::witness::{block_witness::BlockWitness, full_block::FullBlock},
    constants::NUM_SENDERS_IN_BLOCK,
    ethereum_types::{account_id_packed::AccountIdPacked, bytes32::Bytes32, u256::U256},
    utils::trees::indexed_merkle_tree::leaf::IndexedMerkleLeaf,
};

use crate::{
    account_tree::HistoricalAccountTree, block_tree::HistoricalBlockHashTree, node::NodeDB,
};

pub async fn to_block_witness<ADB: NodeDB<IndexedMerkleLeaf>, BDB: NodeDB<Bytes32>>(
    full_block: FullBlock,
    account_tree: &HistoricalAccountTree<ADB>,
    block_tree: &HistoricalBlockHashTree<BDB>,
) -> anyhow::Result<BlockWitness> {
    ensure!(
        full_block.block.block_number != 0,
        "genesis block is not allowed"
    );
    let is_registration_block = full_block.signature.is_registration_block;
    let (pubkeys, account_id_packed, account_merkle_proofs, account_membership_proofs) =
        if is_registration_block {
            let mut pubkeys = full_block.pubkeys.clone().ok_or(anyhow::anyhow!(
                "pubkeys is not given while it is registration block"
            ))?;
            pubkeys.resize(NUM_SENDERS_IN_BLOCK, U256::dummy_pubkey());
            let mut account_membership_proofs = Vec::new();
            for pubkey in pubkeys.iter() {
                let is_dummy = pubkey.is_dummy_pubkey();
                let leaves = account_tree.get_current_leaves().await?;
                ensure!(
                    account_tree.index(&leaves, *pubkey).await?.is_none() || is_dummy,
                    "account already exists"
                );
                let root = account_tree.get_current_root().await?;
                let proof = account_tree.prove_membership_by_root(root, *pubkey).await?;
                account_membership_proofs.push(proof);
            }
            (pubkeys, None, None, Some(account_membership_proofs))
        } else {
            let account_id_trimmed_bytes = full_block.account_ids.clone().ok_or(
                anyhow::anyhow!("account_ids is not given while it is non-registration block"),
            )?;
            let account_id_packed = AccountIdPacked::from_trimmed_bytes(&account_id_trimmed_bytes)
                .map_err(|e| anyhow::anyhow!("error while recovering packed account ids {}", e))?;
            let account_ids = account_id_packed.unpack();
            let mut account_merkle_proofs = Vec::new();
            let mut pubkeys = Vec::new();
            for account_id in account_ids {
                let root = account_tree.get_current_root().await?;
                let pubkey = account_tree.key_by_root(root, account_id).await?;
                let proof = account_tree
                    .prove_inclusion_by_root(root, account_id)
                    .await?;
                pubkeys.push(pubkey);
                account_merkle_proofs.push(proof);
            }
            (
                pubkeys,
                Some(account_id_packed),
                Some(account_merkle_proofs),
                None,
            )
        };
    let prev_account_tree_root = account_tree.get_current_root().await?;
    let prev_block_tree_root = block_tree.get_current_root().await?;
    let block_witness = BlockWitness {
        block: full_block.block.clone(),
        signature: full_block.signature.clone(),
        pubkeys: pubkeys.clone(),
        prev_account_tree_root,
        prev_block_tree_root,
        account_id_packed,
        account_merkle_proofs,
        account_membership_proofs,
    };
    Ok(block_witness)
}
