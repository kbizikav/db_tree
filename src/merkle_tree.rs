use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};

use crate::mock_db::{MockDB, Node};

// `MekleTree`` is a structure of Merkle Tree used for `MerkleTreeWithLeaves`
// and `SparseMerkleTreeWithLeaves`. It only holds non-zero nodes.
// All nodes are specified by path: Vec<bool>. The path is big endian.
// Note that this is different from the original plonky2 Merkle Tree which
// uses little endian path.
#[derive(Clone, Debug)]
pub struct MerkleTree<V: Leafable> {
    height: usize,
    node_hashes: HashMap<Vec<bool>, <V::LeafableHasher as LeafableHasher>::HashOut>,
    zero_hashes: Vec<<V::LeafableHasher as LeafableHasher>::HashOut>,
}

impl<V: Leafable> MerkleTree<V> {
    pub fn new(
        mock_db: &mut MockDB<V>,
        height: usize,
        empty_leaf_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) -> Self {
        // zero_hashes = reverse([H(zero_leaf), H(H(zero_leaf), H(zero_leaf)), ...])
        let mut zero_hashes = vec![];
        let mut h = empty_leaf_hash;
        zero_hashes.push(h.clone());
        for _ in 0..height {
            let new_h = <V::LeafableHasher as LeafableHasher>::two_to_one(h, h);
            zero_hashes.push(new_h);
            mock_db.insert(
                new_h,
                Node {
                    left: Some(h.clone()),
                    right: Some(h.clone()),
                },
            );
            h = new_h;
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
        self.get_node_hash(&path)
    }

    // index_bits is little endian
    pub fn update_leaf(
        &mut self,
        mock_db: &mut MockDB<V>,
        index_bits: Vec<bool>,
        leaf_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) {
        assert_eq!(index_bits.len(), self.height);
        let mut path = index_bits;
        path.reverse(); // path is big endian

        let mut h = leaf_hash;
        self.node_hashes.insert(path.clone(), h.clone()); // leaf node
        mock_db.insert(
            h.clone(),
            Node {
                left: None,
                right: None,
            },
        );

        while !path.is_empty() {
            let sibling = self.get_sibling_hash(&path);
            let b = path.pop().unwrap();
            let new_h = if b {
                <V::LeafableHasher as LeafableHasher>::two_to_one(sibling, h)
            } else {
                <V::LeafableHasher as LeafableHasher>::two_to_one(h, sibling)
            };
            self.node_hashes.insert(path.clone(), new_h.clone());
            let node = Node {
                left: if b { Some(sibling) } else { Some(h.clone()) },
                right: if b { Some(h.clone()) } else { Some(sibling) },
            };
            mock_db.insert(new_h.clone(), node);
            h = new_h;
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

    pub fn prove_with_given_root(
        &self,
        mock_db: &MockDB<V>,
        root: <V::LeafableHasher as LeafableHasher>::HashOut,
        index_bits: Vec<bool>,
    ) -> MerkleProof<V> {
        assert_eq!(index_bits.len(), self.height);
        let mut path = index_bits;
        let mut siblings = vec![];
        let mut hash = root;
        while !path.is_empty() {
            let node = mock_db.get(hash).expect("cannot find node");
            let (child, sibling) = if path.pop().unwrap() {
                (node.right.unwrap(), node.left.unwrap())
            } else {
                (node.left.unwrap(), node.right.unwrap())
            };
            siblings.push(sibling);
            hash = child;
        }
        siblings.reverse();
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

pub fn usize_le_bits(num: usize, length: usize) -> Vec<bool> {
    let mut result = Vec::with_capacity(length);
    let mut n = num;
    for _ in 0..length {
        result.push(n & 1 == 1);
        n >>= 1;
    }
    result
}

#[cfg(test)]
mod test {
    use intmax2_zkp::utils::{leafable::Leafable, poseidon_hash_out::PoseidonHashOut};

    use crate::{merkle_tree::usize_le_bits, mock_db::MockDB};

    use super::MerkleTree;

    type Leaf = u32;

    #[test]
    fn test_prove_with_given_root() {
        let height = 32;

        let mut mock_db = MockDB::<Leaf>::new();
        let empty_leaf_hash = PoseidonHashOut::hash_inputs_u32(&[]);
        let mut merkle_tree = MerkleTree::new(&mut mock_db, height, empty_leaf_hash);

        for i in 0..10 {
            let leaf = i as u32;
            let index_bits = super::usize_le_bits(i, height);
            merkle_tree.update_leaf(&mut mock_db, index_bits, leaf.hash());
        }
        let root1 = merkle_tree.get_root();
        for i in 10..20 {
            let leaf_hash = PoseidonHashOut::hash_inputs_u32(&[i as u32]);
            let index_bits = usize_le_bits(i, height);
            merkle_tree.update_leaf(&mut mock_db, index_bits, leaf_hash);
        }
        let index = 6;
        let leaf = index as u32;
        let index_bits = super::usize_le_bits(index, height);
        let proof = merkle_tree.prove_with_given_root(&mock_db, root1, index_bits.clone());
        let root1_expected = proof.get_root(&leaf, index_bits);
        assert_eq!(root1, root1_expected);
    }
}
