use std::collections::HashMap;

use intmax2_zkp::utils::{
    leafable::{Leafable, LeafableTarget},
    leafable_hasher::LeafableHasher,
};
use plonky2::{
    field::{extension::Extendable, types::Field},
    hash::hash_types::RichField,
    iop::{target::BoolTarget, witness::WitnessWrite},
    plonk::{
        circuit_builder::CircuitBuilder,
        config::{AlgebraicHasher, GenericConfig},
    },
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// `MekleTree`` is a structure of Merkle Tree used for `MerkleTreeWithLeaves`
// and `SparseMerkleTreeWithLeaves`. It only holds non-zero nodes.
// All nodes are specified by path: Vec<bool>. The path is big endian.
// Note that this is different from the original plonky2 Merkle Tree which
// uses little endian path.
#[derive(Clone, Debug)]
pub struct MerkleTree<V: Leafable> {
    pub height: usize,
    pub node_hashes: HashMap<Vec<bool>, <V::LeafableHasher as LeafableHasher>::HashOut>,
    pub zero_hashes: Vec<<V::LeafableHasher as LeafableHasher>::HashOut>,
}

impl<V: Leafable> MerkleTree<V> {
    pub fn new(
        height: usize,
        empty_leaf_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) -> Self {
        // zero_hashes = reverse([H(zero_leaf), H(H(zero_leaf), H(zero_leaf)), ...])
        let mut zero_hashes = vec![];
        let mut h = empty_leaf_hash;
        zero_hashes.push(h.clone());
        for _ in 0..height {
            h = <V::LeafableHasher as LeafableHasher>::two_to_one(h, h);
            zero_hashes.push(h.clone());
        }
        zero_hashes.reverse();

        let node_hashes: HashMap<Vec<bool>, <V::LeafableHasher as LeafableHasher>::HashOut> =
            HashMap::new();

        Self {
            height,
            node_hashes,
            zero_hashes,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get_node_hash(
        &self,
        path: &Vec<bool>,
    ) -> <V::LeafableHasher as LeafableHasher>::HashOut {
        assert!(path.len() <= self.height);
        match self.node_hashes.get(path) {
            Some(h) => h.clone(),
            None => self.zero_hashes[path.len()].clone(),
        }
    }

    pub fn get_root(&self) -> <V::LeafableHasher as LeafableHasher>::HashOut {
        self.get_node_hash(&vec![])
    }

    fn get_sibling_hash(&self, path: &Vec<bool>) -> <V::LeafableHasher as LeafableHasher>::HashOut {
        assert!(!path.is_empty());
        let mut path = path.clone();
        let last = path.len() - 1;
        path[last] = !path[last];
        println!("sibling path: {:?}", path);
        let node_hash = self.get_node_hash(&path);
        dbg!(&node_hash);
        node_hash
    }

    // index_bits is little endian
    pub fn update_leaf(
        &mut self,
        index_bits: Vec<bool>,
        leaf_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) {
        assert_eq!(index_bits.len(), self.height);
        let mut path = index_bits;
        path.reverse(); // path is big endian

        let mut h = leaf_hash;
        self.node_hashes.insert(path.clone(), h.clone());

        while !path.is_empty() {
            let sibling = self.get_sibling_hash(&path);
            h = if path.pop().unwrap() {
                <V::LeafableHasher as LeafableHasher>::two_to_one(sibling, h)
            } else {
                <V::LeafableHasher as LeafableHasher>::two_to_one(h, sibling)
            };
            self.node_hashes.insert(path.clone(), h.clone());
        }
    }

    pub fn prove(&self, index_bits: Vec<bool>) -> MerkleProof<V> {
        assert_eq!(index_bits.len(), self.height);
        let mut path = index_bits;
        path.reverse(); // path is big endian

        let mut siblings = vec![];
        while !path.is_empty() {
            siblings.push(self.get_sibling_hash(&path));
            path.pop();
        }
        MerkleProof { siblings }
    }
}

#[derive(Clone, Debug)]
pub struct MerkleProof<V: Leafable> {
    pub siblings: Vec<<V::LeafableHasher as LeafableHasher>::HashOut>,
}

impl<V: Leafable> Serialize for MerkleProof<V>
where
    <V::LeafableHasher as LeafableHasher>::HashOut: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.siblings.serialize(serializer)
    }
}

impl<'de, V: Leafable> Deserialize<'de> for MerkleProof<V>
where
    <V::LeafableHasher as LeafableHasher>::HashOut: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let siblings =
            Vec::<<V::LeafableHasher as LeafableHasher>::HashOut>::deserialize(deserializer)?;
        Ok(MerkleProof { siblings })
    }
}

impl<V: Leafable> MerkleProof<V> {
    pub fn dummy(height: usize) -> Self {
        Self {
            siblings: vec![<V::LeafableHasher as LeafableHasher>::HashOut::default(); height],
        }
    }

    pub fn height(&self) -> usize {
        self.siblings.len()
    }

    pub fn get_root(
        &self,
        leaf_data: &V,
        index_bits: Vec<bool>,
    ) -> <V::LeafableHasher as LeafableHasher>::HashOut {
        let mut state = leaf_data.hash();
        for (&bit, sibling) in index_bits.iter().zip(self.siblings.iter()) {
            state = if bit {
                <V::LeafableHasher as LeafableHasher>::two_to_one(*sibling, state)
            } else {
                <V::LeafableHasher as LeafableHasher>::two_to_one(state, *sibling)
            }
        }
        state
    }

    pub fn verify(
        &self,
        leaf_data: &V,
        index_bits: Vec<bool>, // little endian
        merkle_root: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.get_root(leaf_data, index_bits) == merkle_root,
            "Merkle proof verification failed"
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MerkleProofTarget<VT: LeafableTarget> {
    pub siblings: Vec<<<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::HashOutTarget>,
}

impl<VT: LeafableTarget> PartialEq for MerkleProofTarget<VT>
where
    <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::HashOutTarget: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        if self.siblings.len() != other.siblings.len() {
            return false;
        }
        for (a, b) in self.siblings.iter().zip(other.siblings.iter()) {
            if a != b {
                return false;
            }
        }

        true
    }
}

impl<VT: LeafableTarget> Eq for MerkleProofTarget<VT>
where
    <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::HashOutTarget: Eq,
{
    // Nothing to implement
}

impl<VT: LeafableTarget> MerkleProofTarget<VT> {
    pub fn new<F: RichField + Extendable<D>, const D: usize>(
        builder: &mut CircuitBuilder<F, D>,
        height: usize,
    ) -> Self {
        let siblings = (0..height)
            .map(|_| {
                <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::hash_out_target(builder)
            })
            .collect::<Vec<_>>();
        Self { siblings }
    }

    pub fn constant<F: RichField + Extendable<D>, const D: usize>(
        builder: &mut CircuitBuilder<F, D>,
        input: &MerkleProof<VT::Leaf>,
    ) -> Self {
        Self {
            siblings: input
                .siblings
                .iter()
                .map(|sibling| <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::constant_hash_out_target(builder, *sibling))
                .collect(),
        }
    }

    pub fn set_witness<F: Field, W: WitnessWrite<F>>(
        &self,
        pw: &mut W,
        merkle_proof: &MerkleProof<VT::Leaf>,
    ) {
        assert_eq!(self.siblings.len(), merkle_proof.siblings.len());
        for (sibling_t, sibling) in self.siblings.iter().zip(merkle_proof.siblings.iter()) {
            <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::set_hash_out_target(
                sibling_t, pw, *sibling,
            );
        }
    }
}

impl<VT: LeafableTarget> MerkleProofTarget<VT> {
    pub fn get_root<
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F> + 'static,
        const D: usize,
    >(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        leaf_data: &VT,
        index_bits: Vec<BoolTarget>,
    ) -> <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::HashOutTarget
    where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
    {
        let mut state = leaf_data.hash::<F, C, D>(builder);
        assert_eq!(index_bits.len(), self.siblings.len());
        for (bit, sibling) in index_bits.iter().zip(&self.siblings) {
            state = <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::two_to_one_swapped::<
                F,
                C,
                D,
            >(builder, &state, sibling, *bit);
        }
        state
    }

    pub fn verify<
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F> + 'static,
        const D: usize,
    >(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        leaf_data: &VT,
        index_bits: Vec<BoolTarget>,
        merkle_root: <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::HashOutTarget,
    ) where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
    {
        let state = self.get_root::<F, C, D>(builder, leaf_data, index_bits);
        <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::connect_hash(
            builder,
            &state,
            &merkle_root,
        );
    }

    pub fn conditional_verify<
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F> + 'static,
        const D: usize,
    >(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        condition: BoolTarget,
        leaf_data: &VT,
        index_bits: Vec<BoolTarget>,
        merkle_root: <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::HashOutTarget,
    ) where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
    {
        let state = self.get_root::<F, C, D>(builder, leaf_data, index_bits);
        <<VT::Leaf as Leafable>::LeafableHasher as LeafableHasher>::conditional_assert_eq_hash(
            builder,
            condition,
            &state,
            &merkle_root,
        );
    }

    pub fn height(&self) -> usize {
        self.siblings.len()
    }
}

pub fn u64_le_bits(num: u64, length: usize) -> Vec<bool> {
    let mut result = Vec::with_capacity(length);
    let mut n = num;
    for _ in 0..length {
        result.push(n & 1 == 1);
        n >>= 1;
    }
    result
}

#[cfg(test)]
mod tests {

    use super::*;
    use intmax2_zkp::ethereum_types::{bytes32::Bytes32, u32limb_trait::U32LimbTrait};

    use rand::Rng;

    #[test]
    fn merkle_tree_update_prove_verify() {
        type V = Bytes32;

        let mut rng = rand::thread_rng();
        let height = 10;
        let empty_leaf_hash = V::default().hash();
        let mut tree = MerkleTree::<Bytes32>::new(height, empty_leaf_hash);

        for _ in 0..100 {
            let index = rng.gen_range(0..1 << height);
            let new_leaf = Bytes32::rand(&mut rng);
            let leaf_hash = new_leaf.hash();
            let index_bits = u64_le_bits(index, height);
            tree.update_leaf(index_bits.clone(), leaf_hash);
            let proof = tree.prove(index_bits.clone());
            proof
                .verify(&new_leaf, index_bits, tree.get_root())
                .unwrap();
        }
    }
}
