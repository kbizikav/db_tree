use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};

use crate::mock_db::{MockDB, Node};

type Hasher<V> = <V as Leafable>::LeafableHasher;
type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;

// `MekleTree`` is a structure of Merkle Tree used for `MerkleTreeWithLeaves`
// and `SparseMerkleTreeWithLeaves`. It only holds non-zero nodes.
// All nodes are specified by path: Vec<bool>. The path is big endian.
// Note that this is different from the original plonky2 Merkle Tree which
// uses little endian path.
#[derive(Clone, Debug)]
pub struct MerkleTree<V: Leafable> {
    height: usize,
    node_hashes: HashMap<Vec<bool>, HashOut<V>>,
    zero_hashes: Vec<HashOut<V>>,
}

impl<V: Leafable> MerkleTree<V> {
    pub async fn new(mock_db: &MockDB<V>, height: usize) -> Self {
        // zero_hashes = reverse([H(zero_leaf), H(H(zero_leaf), H(zero_leaf)), ...])
        let mut zero_hashes = vec![];
        let mut h = V::empty_leaf().hash();
        zero_hashes.push(h.clone());
        for _ in 0..height {
            let new_h = Hasher::<V>::two_to_one(h, h);
            zero_hashes.push(new_h);
            mock_db
                .insert(
                    new_h,
                    Node {
                        left_hash: h.clone(),
                        right_hash: h.clone(),
                    },
                )
                .await;
            h = new_h;
        }
        zero_hashes.reverse();

        let node_hashes: HashMap<Vec<bool>, HashOut<V>> = HashMap::new();

        Self {
            height,
            node_hashes,
            zero_hashes,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get_node_hash(&self, path: &Vec<bool>) -> HashOut<V> {
        assert!(path.len() <= self.height);
        match self.node_hashes.get(path) {
            Some(h) => h.clone(),
            None => self.zero_hashes[path.len()].clone(),
        }
    }

    pub fn get_root(&self) -> HashOut<V> {
        self.get_node_hash(&vec![])
    }

    fn get_sibling_hash(&self, path: &Vec<bool>) -> HashOut<V> {
        assert!(!path.is_empty());
        let mut path = path.clone();
        let last = path.len() - 1;
        path[last] = !path[last];
        self.get_node_hash(&path)
    }

    // index_bits is little endian
    pub async fn update_leaf(&mut self, mock_db: &MockDB<V>, index: u64, leaf_hash: HashOut<V>) {
        let mut path = u64_le_bits(index, self.height());
        path.reverse(); // path is big endian

        let mut h = leaf_hash;
        self.node_hashes.insert(path.clone(), h.clone()); // leaf node

        while !path.is_empty() {
            let sibling = self.get_sibling_hash(&path);
            let b = path.pop().unwrap();
            let new_h = if b {
                Hasher::<V>::two_to_one(sibling, h)
            } else {
                Hasher::<V>::two_to_one(h, sibling)
            };
            self.node_hashes.insert(path.clone(), new_h.clone());
            let node = Node {
                left_hash: if b { sibling } else { h.clone() },
                right_hash: if b { h.clone() } else { sibling },
            };
            mock_db.insert(new_h.clone(), node).await;
            h = new_h;
        }
    }

    pub async fn prove_with_given_root(
        &self,
        mock_db: &MockDB<V>,
        root: HashOut<V>,
        index: u64,
    ) -> MerkleProof<V> {
        let mut path = u64_le_bits(index, self.height());
        let mut siblings = vec![];
        let mut hash = root;
        while !path.is_empty() {
            let node = mock_db.get(hash).await.expect("cannot find node");
            let (child, sibling) = if path.pop().unwrap() {
                (node.right_hash, node.left_hash)
            } else {
                (node.left_hash, node.right_hash)
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
    pub siblings: Vec<HashOut<V>>,
}

impl<V: Leafable> Serialize for MerkleProof<V>
where
    HashOut<V>: Serialize,
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
    HashOut<V>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let siblings = Vec::<HashOut<V>>::deserialize(deserializer)?;
        Ok(MerkleProof { siblings })
    }
}

impl<V: Leafable> MerkleProof<V> {
    pub fn dummy(height: usize) -> Self {
        Self {
            siblings: vec![HashOut::<V>::default(); height],
        }
    }

    pub fn height(&self) -> usize {
        self.siblings.len()
    }

    pub fn get_root(&self, leaf_data: &V, index: u64) -> HashOut<V> {
        let mut state = leaf_data.hash();
        let index_bits = u64_le_bits(index, self.height());
        for (&bit, sibling) in index_bits.iter().zip(self.siblings.iter()) {
            state = if bit {
                Hasher::<V>::two_to_one(*sibling, state)
            } else {
                Hasher::<V>::two_to_one(state, *sibling)
            }
        }
        state
    }

    pub fn verify(&self, leaf_data: &V, index: u64, merkle_root: HashOut<V>) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.get_root(leaf_data, index) == merkle_root,
            "Merkle proof verification failed"
        );
        Ok(())
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
mod test {
    use intmax2_zkp::utils::{leafable::Leafable, poseidon_hash_out::PoseidonHashOut};

    use crate::mock_db::MockDB;

    use super::MerkleTree;

    type Leaf = u32;

    #[tokio::test]
    async fn test_prove_with_given_root() {
        let height = 32;

        let mock_db = MockDB::<Leaf>::new();
        let mut merkle_tree = MerkleTree::new(&mock_db, height).await;

        for i in 0..10 {
            let leaf = i as u32;
            merkle_tree.update_leaf(&mock_db, i, leaf.hash()).await;
        }
        let root1 = merkle_tree.get_root();
        for i in 10..20 {
            let leaf_hash = PoseidonHashOut::hash_inputs_u32(&[i as u32]);
            merkle_tree.update_leaf(&mock_db, i, leaf_hash).await;
        }
        let index = 6;
        let leaf = index as u32;
        let proof = merkle_tree
            .prove_with_given_root(&mock_db, root1, index)
            .await;
        proof.verify(&leaf, index, root1).unwrap();
        let root1_expected = proof.get_root(&leaf, index);
        assert_eq!(root1, root1_expected);
    }
}
