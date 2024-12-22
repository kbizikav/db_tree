use std::sync::Arc;

use hashbrown::HashMap;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use tokio::sync::RwLock;

type Hasher<V> = <V as Leafable>::LeafableHasher;
type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;

#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left_hash: HashOut<V>,
    pub right_hash: HashOut<V>,
}

pub trait NodeDB<V: Leafable> {}

#[derive(Clone, Debug)]
pub struct MockDB<V: Leafable> {
    nodes: Arc<RwLock<HashMap<HashOut<V>, Node<V>>>>, // parents hash to node (2 child hashes)
}

impl<V: Leafable> MockDB<V> {
    pub fn new() -> Self {
        MockDB {
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) {
        self.nodes.write().await.insert(parent_hash, node);
    }

    pub async fn get(&self, parent_hash: HashOut<V>) -> Option<Node<V>> {
        self.nodes.read().await.get(&parent_hash).cloned()
    }
}
