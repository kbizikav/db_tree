use hashbrown::HashMap;
use intmax2_zkp::utils::poseidon_hash_out::PoseidonHashOut;

#[derive(Clone, Debug)]
pub struct Node {
    pub left: PoseidonHashOut,
    pub right: PoseidonHashOut,
}

#[derive(Clone, Debug)]
pub struct MockTree {
    nodes: HashMap<PoseidonHashOut, Node>, // parents hash to node (2 child hashes)
}

impl MockTree {
    pub fn new() -> Self {
        MockTree {
            nodes: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: PoseidonHashOut, node: Node) {
        self.nodes.insert(key, node);
    }
}
