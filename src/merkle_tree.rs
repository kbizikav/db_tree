use std::collections::HashMap;

use intmax2_zkp::utils::{
    leafable::Leafable, leafable_hasher::LeafableHasher, trees::merkle_tree::MerkleProof,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    bit_path::BitPath,
    error::HistoricalMerkleTreeError,
    node::{Node, NodeDB},
};

pub type Hasher<V> = <V as Leafable>::LeafableHasher;
pub type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;
pub type HMTResult<T> = Result<T, HistoricalMerkleTreeError>;

#[derive(Clone, Debug)]
pub struct HistoricalMerkleTree<V: Leafable + Serialize + DeserializeOwned, DB: NodeDB<V>> {
    height: u32,
    node_hashes: HashMap<BitPath, HashOut<V>>,
    zero_hashes: Vec<HashOut<V>>,
    node_db: DB,
}

impl<V: Leafable + Serialize + DeserializeOwned, DB: NodeDB<V>> HistoricalMerkleTree<V, DB> {
    pub async fn new(node_db: DB, height: u32) -> HMTResult<Self> {
        let zero_hashes = Self::init_zero_hashes(height, &node_db).await?;
        let node_hashes: HashMap<BitPath, HashOut<V>> = HashMap::new();
        Ok(Self {
            height,
            node_hashes,
            zero_hashes,
            node_db,
        })
    }

    pub async fn load(&mut self) -> HMTResult<()> {
        let time = std::time::Instant::now();
        let leaf_hashes = self.node_db.get_all_leaf_hashes().await?;
        for (index, leaf_hash) in leaf_hashes {
            self.update_leaf(false, index, leaf_hash).await?;
        }
        tracing::info!("load time: {:?}", time.elapsed());
        Ok(())
    }

    pub fn node_db(&self) -> &DB {
        &self.node_db
    }

    async fn init_zero_hashes(height: u32, node_db: &DB) -> HMTResult<Vec<HashOut<V>>> {
        // zero_hashes = reverse([H(zero_leaf), H(H(zero_leaf), H(zero_leaf)), ...])
        let mut zero_hashes = vec![];
        let mut h = V::empty_leaf().hash();
        zero_hashes.push(h.clone());
        for _ in 0..height {
            let new_h = Hasher::<V>::two_to_one(h, h);
            zero_hashes.push(new_h);
            node_db
                .insert(
                    new_h,
                    Node {
                        left_hash: h,
                        right_hash: h,
                    },
                )
                .await?;
            h = new_h;
        }
        zero_hashes.reverse();
        Ok(zero_hashes)
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn get_root(&self) -> HMTResult<HashOut<V>> {
        self.get_node_hash(BitPath::default())
    }

    fn get_node_hash(&self, path: BitPath) -> HMTResult<HashOut<V>> {
        if path.len() > self.height {
            return Err(HistoricalMerkleTreeError::WrongPathLength(path.len() as u32));
        }
        let hash = match self.node_hashes.get(&path) {
            Some(h) => h.clone(),
            None => self.zero_hashes[path.len() as usize].clone(),
        };
        Ok(hash)
    }

    fn get_sibling_hash(&self, path: BitPath) -> HMTResult<HashOut<V>> {
        if path.is_empty() {
            return Err(HistoricalMerkleTreeError::WrongPathLength(0));
        }
        self.get_node_hash(path.sibling())
    }

    pub async fn update_leaf(
        &mut self,
        update_db: bool,
        index: u64,
        leaf_hash: HashOut<V>,
    ) -> HMTResult<()> {
        let mut path = BitPath::new(self.height(), index);
        path.reverse();
        let mut h = leaf_hash;
        self.node_hashes.insert(path.clone(), h.clone());

        while !path.is_empty() {
            let sibling = self.get_sibling_hash(path)?;
            let b = path.pop().unwrap(); // safe to unwrap
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
            self.node_db.insert(new_h.clone(), node).await?;
            h = new_h;
        }
        if update_db {
            self.node_db.insert_leaf_hash(index, leaf_hash).await?;
        }
        Ok(())
    }

    pub async fn prove_by_root(
        &self,
        root: HashOut<V>,
        index: u64,
        leaf: HashOut<V>,
    ) -> HMTResult<MerkleProof<V>> {
        let mut path = BitPath::new(self.height(), index);
        let mut siblings = vec![];
        let mut hash = root;
        while !path.is_empty() {
            let node = self.node_db.get(hash).await?.ok_or_else(|| {
                HistoricalMerkleTreeError::NodeNotFoundError(format!("{:?}", hash))
            })?;
            let (child, sibling) = if path.pop().unwrap() {
                (node.right_hash, node.left_hash)
            } else {
                (node.left_hash, node.right_hash)
            };
            siblings.push(sibling);
            hash = child;
        }
        if leaf != hash {
            return Err(HistoricalMerkleTreeError::LeafHashMismatch {
                expected: format!("{:?}", leaf),
                got: format!("{:?}", hash),
            });
        }
        siblings.reverse();
        Ok(MerkleProof { siblings })
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
    use intmax2_zkp::utils::leafable::Leafable;
    use rand::Rng;
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

    use crate::node::{NodeDB, SqlNodeDB};

    use super::HistoricalMerkleTree;

    type Leaf = u32;

    #[tokio::test]
    async fn test_prove_with_given_root() -> anyhow::Result<()> {
        let height = 32;
        dotenv::dotenv().ok();

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer().pretty().with_filter(
                    EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()),
                ),
            )
            .try_init()
            .unwrap();

        let mut rng = rand::thread_rng();
        let database_url = std::env::var("DATABASE_URL")?;
        let tag = 0;

        let node_db = SqlNodeDB::<Leaf>::new(&database_url, tag).await?;
        // node_db.reset().await?;
        let mut merkle_tree = HistoricalMerkleTree::new(node_db, height).await?;
        merkle_tree.load().await?;

        let num_leaves = merkle_tree.node_db.get_all_leaf_hashes().await?.len() as u64;
        for i in num_leaves..num_leaves + 10 {
            let leaf = i as u32;
            merkle_tree.update_leaf(true, i, leaf.hash()).await?;
        }
        let root1 = merkle_tree.get_root()?;
        for i in num_leaves + 10..num_leaves + 20 {
            let leaf = i as u32;
            merkle_tree.update_leaf(true, i, leaf.hash()).await?;
        }
        let index = rng.gen_range(0..num_leaves + 10);
        let leaf = index as u32;
        assert_eq!(
            leaf.hash(),
            merkle_tree
                .node_db
                .get_leaf_hash(index)
                .await?
                .expect("leaf not found")
        );
        let proof = merkle_tree.prove_by_root(root1, index, leaf.hash()).await?;
        let index_bits = super::u64_le_bits(index, height as usize);
        proof.verify(&leaf, index_bits, root1)?;

        Ok(())
    }
}
