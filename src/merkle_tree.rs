use hashbrown::HashMap;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::node_db::{Node, NodeDB};

type Hasher<V> = <V as Leafable>::LeafableHasher;
type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;

#[derive(Clone, Debug)]
pub struct MerkleTree<V: Leafable> {
    height: usize,
    node_hashes: HashMap<Vec<bool>, HashOut<V>>,
    zero_hashes: Vec<HashOut<V>>,
    node_db: NodeDB<V>,
}

impl<V: Leafable> MerkleTree<V> {
    pub async fn new(
        height: usize,
        empty_leaf_hash: HashOut<V>,
        node_db: NodeDB<V>,
    ) -> anyhow::Result<Self> {
        let zero_hashes = Self::init_zero_hashes(height, empty_leaf_hash, &node_db).await?;
        let leah_hashes = node_db.get_all_leaf_hashes().await.unwrap();
        let node_hashes: HashMap<Vec<bool>, HashOut<V>> = HashMap::new();
        let mut tree = Self {
            height,
            zero_hashes,
            node_hashes,
            node_db,
        };
        // Insert the leaf hashes
        for (index, leaf_hash) in leah_hashes.iter() {
            let index_bits = u64_le_bits(*index, height);
            tree.update_leaf(index_bits, leaf_hash.clone()).await;
        }
        Ok(tree)
    }

    async fn init_zero_hashes(
        height: usize,
        empty_leaf_hash: HashOut<V>,
        node_db: &NodeDB<V>,
    ) -> anyhow::Result<Vec<HashOut<V>>> {
        // zero_hashes = reverse([H(zero_leaf), H(H(zero_leaf), H(zero_leaf)), ...])
        let mut zero_hashes = vec![];
        let mut h = empty_leaf_hash;
        zero_hashes.push(h.clone());
        for _ in 0..height {
            let new_h = Hasher::<V>::two_to_one(h, h);
            zero_hashes.push(new_h);
            node_db
                .insert(new_h, Node { left: h, right: h })
                .await
                .unwrap();
            h = new_h;
        }
        zero_hashes.reverse();
        Ok(zero_hashes)
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
    pub async fn update_leaf(&mut self, index_bits: Vec<bool>, leaf_hash: HashOut<V>) {
        assert_eq!(index_bits.len(), self.height);
        let mut path = index_bits;
        path.reverse(); // path is big endian

        let mut h = leaf_hash;
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
                left: if b { sibling } else { h.clone() },
                right: if b { h.clone() } else { sibling },
            };
            self.node_db.insert(new_h.clone(), node).await.unwrap();
            h = new_h;
        }
    }

    pub async fn prove_with_given_root(
        &self,
        root: HashOut<V>,
        index_bits: Vec<bool>,
    ) -> MerkleProof<V> {
        assert_eq!(index_bits.len(), self.height);
        let mut path = index_bits;
        let mut siblings = vec![];
        let mut hash = root;
        while !path.is_empty() {
            let node = self
                .node_db
                .get(hash)
                .await
                .unwrap()
                .expect("cannot find node");
            let (child, sibling) = if path.pop().unwrap() {
                (node.right, node.left)
            } else {
                (node.left, node.right)
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

    pub fn get_root(&self, leaf_data: &V, index_bits: Vec<bool>) -> HashOut<V> {
        let mut state = leaf_data.hash();
        for (&bit, sibling) in index_bits.iter().zip(self.siblings.iter()) {
            state = if bit {
                Hasher::<V>::two_to_one(*sibling, state)
            } else {
                Hasher::<V>::two_to_one(state, *sibling)
            }
        }
        state
    }

    pub fn verify(
        &self,
        leaf_data: &V,
        index_bits: Vec<bool>, // little endian
        merkle_root: HashOut<V>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.get_root(leaf_data, index_bits) == merkle_root,
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

// #[cfg(test)]
// mod test {
//     use intmax2_zkp::utils::{leafable::Leafable, poseidon_hash_out::PoseidonHashOut};

//     use crate::{merkle_tree::usize_le_bits, node_db::MockDB};

//     use super::MerkleTree;

//     type Leaf = u32;

//     #[test]
//     fn test_prove_with_given_root() {
//         let height = 32;

//         let mut mock_db = MockDB::<Leaf>::new();
//         let empty_leaf_hash = PoseidonHashOut::hash_inputs_u32(&[]);
//         let mut merkle_tree = MerkleTree::new(&mut mock_db, height, empty_leaf_hash);

//         for i in 0..10 {
//             let leaf = i as u32;
//             let index_bits = super::usize_le_bits(i, height);
//             merkle_tree.update_leaf(&mut mock_db, index_bits, leaf.hash());
//         }
//         let root1 = merkle_tree.get_root();
//         for i in 10..20 {
//             let leaf_hash = PoseidonHashOut::hash_inputs_u32(&[i as u32]);
//             let index_bits = usize_le_bits(i, height);
//             merkle_tree.update_leaf(&mut mock_db, index_bits, leaf_hash);
//         }
//         let index = 6;
//         let leaf = index as u32;
//         let index_bits = super::usize_le_bits(index, height);
//         let proof = merkle_tree.prove_with_given_root(&mock_db, root1, index_bits.clone());
//         let root1_expected = proof.get_root(&leaf, index_bits);
//         assert_eq!(root1, root1_expected);
//     }
// }
