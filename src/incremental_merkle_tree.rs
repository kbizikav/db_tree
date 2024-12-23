use plonky2::{
    field::{extension::Extendable, types::Field},
    hash::{hash_types::RichField, merkle_tree::MerkleTree},
    iop::{
        target::{BoolTarget, Target},
        witness::WitnessWrite,
    },
    plonk::{
        circuit_builder::CircuitBuilder,
        config::{AlgebraicHasher, GenericConfig},
    },
};

use intmax2_zkp::utils::{
    leafable::Leafable, leafable_hasher::LeafableHasher, trees::merkle_tree::u64_le_bits,
};

use crate::{merkle_tree::HistoricalMerkleTree, node::NodeDB};


#[derive(Debug, Clone)]
pub struct HistoricalIncrementalMerkleTree<V: Leafable, DB: NodeDB<V>>(HistoricalMerkleTree<V, DB>);

impl<V: Leafable, DB: NodeDB<V>> HistoricalIncrementalMerkleTree<V> {
    pub fn new(height: usize) -> Self {
        let merkle_tree = MerkleTree::new(height, V::empty_leaf().hash());
        let leaves = vec![];

        Self {
            merkle_tree,
            leaves,
        }
    }

    pub fn height(&self) -> usize {
        self.merkle_tree.height()
    }

    // NOTICE: `None` and `V::empty_leaf()` are treated equivalently.
    pub fn get_leaf(&self, index: u64) -> V {
        match self.leaves.get(index as usize) {
            Some(leaf) => leaf.clone(),
            None => V::empty_leaf(),
        }
    }

    pub fn get_root(&self) -> <V::LeafableHasher as LeafableHasher>::HashOut {
        self.merkle_tree.get_root()
    }

    pub fn leaves(&self) -> Vec<V> {
        self.leaves.clone()
    }

    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    pub fn update(&mut self, index: u64, leaf: V) {
        let index_bits = u64_le_bits(index, self.height());
        self.merkle_tree.update_leaf(index_bits, leaf.hash());
        self.leaves[index as usize] = leaf;
    }

    pub fn push(&mut self, leaf: V) {
        let index = self.leaves.len() as u64;
        assert!(index < (1u64 << (self.height() as u64)));
        let leaf_hash = leaf.hash();
        self.leaves.push(leaf);
        let index_bits = u64_le_bits(index, self.height());
        self.merkle_tree.update_leaf(index_bits, leaf_hash);
    }

    pub fn pop(&mut self) {
        assert!(!self.leaves.is_empty());
        self.leaves.pop();
        let index = self.leaves.len() as u64;
        let leaf = V::empty_leaf();
        let index_bits = u64_le_bits(index, self.height());
        self.merkle_tree.update_leaf(index_bits, leaf.hash());
    }

    pub fn prove(&self, index: u64) -> IncrementalMerkleProof<V> {
        let index_bits = u64_le_bits(index, self.height());
        IncrementalMerkleProof(self.merkle_tree.prove(index_bits))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ethereum_types::{
            bytes32::{Bytes32, Bytes32Target},
            u32limb_trait::{U32LimbTargetTrait, U32LimbTrait as _},
        },
        utils::poseidon_hash_out::PoseidonHashOutTarget,
    };

    use super::*;
    use plonky2::{
        field::types::Field,
        iop::witness::{PartialWitness, WitnessWrite},
        plonk::{
            circuit_data::CircuitConfig,
            config::{GenericConfig, PoseidonGoldilocksConfig},
        },
    };
    use rand::Rng;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;

    #[test]
    fn merkle_tree_with_leaves() {
        let mut rng = rand::thread_rng();
        let height = 10;

        type V = Bytes32;
        let mut tree = HistoricalIncrementalMerkleTree::<V>::new(height);

        for _ in 0..100 {
            let new_leaf = Bytes32::rand(&mut rng);
            tree.push(new_leaf);
        }

        for _ in 0..100 {
            let index = rng.gen_range(0..1 << height);
            let leaf = tree.get_leaf(index);
            let proof = tree.prove(index);
            assert_eq!(tree.get_leaf(index), leaf.clone());
            proof.verify(&leaf, index, tree.get_root()).unwrap();
        }
    }

    #[test]
    fn merkle_tree_with_leaves_circuit() {
        let mut rng = rand::thread_rng();
        let height = 10;

        type V = Bytes32;
        type VT = Bytes32Target;
        let mut tree = HistoricalIncrementalMerkleTree::<V>::new(height);
        for _ in 0..1 << height {
            let new_leaf = V::rand(&mut rng);
            tree.push(new_leaf);
        }

        let index = rng.gen_range(0..1 << height);
        let leaf = tree.get_leaf(index);
        let proof = tree.prove(index);

        let mut builder = CircuitBuilder::<F, D>::new(CircuitConfig::default());
        let proof_t = IncrementalMerkleProofTarget::<VT>::new(&mut builder, height);
        let leaf_t = VT::new(&mut builder, false);
        let root_t = PoseidonHashOutTarget::new(&mut builder);
        let index_t = builder.add_virtual_target();
        proof_t.verify::<F, C, D>(&mut builder, &leaf_t, index_t, root_t);

        let data = builder.build::<C>();
        let mut pw = PartialWitness::<F>::new();
        leaf_t.set_witness(&mut pw, leaf);
        root_t.set_witness(&mut pw, tree.get_root());
        pw.set_target(index_t, F::from_canonical_u64(index));
        proof_t.set_witness(&mut pw, &proof);
        data.prove(pw).unwrap();
    }
}
