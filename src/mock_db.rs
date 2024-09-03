use hashbrown::HashMap;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};

#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left: <V::LeafableHasher as LeafableHasher>::HashOut,
    pub right: <V::LeafableHasher as LeafableHasher>::HashOut,
}

#[derive(Clone, Debug)]
pub struct MockDB<V: Leafable> {
    nodes: HashMap<<V::LeafableHasher as LeafableHasher>::HashOut, Node<V>>, // parents hash to node (2 child hashes)
}

impl<V: Leafable> MockDB<V> {
    pub fn new() -> Self {
        MockDB {
            nodes: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: <V::LeafableHasher as LeafableHasher>::HashOut, node: Node<V>) {
        self.nodes.insert(key, node);
    }

    pub fn get(&self, key: <V::LeafableHasher as LeafableHasher>::HashOut) -> Option<Node<V>> {
        self.nodes.get(&key).cloned()
    }
}
