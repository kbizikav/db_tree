use std::sync::Arc;

use async_trait::async_trait;
use hashbrown::HashMap;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::sync::RwLock;

type Hasher<V> = <V as Leafable>::LeafableHasher;
type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;

#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left_hash: HashOut<V>,
    pub right_hash: HashOut<V>,
}

#[async_trait(?Send)]
pub trait NodeDB<V: Leafable>: std::fmt::Debug + Clone {
    async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> anyhow::Result<()>;

    async fn get(&self, parent_hash: HashOut<V>) -> anyhow::Result<Option<Node<V>>>;

    async fn insert_leaf_hash(&self, position: u64, leaf_hash: HashOut<V>) -> anyhow::Result<()>;

    async fn get_all_leaf_hashes(&self) -> anyhow::Result<Vec<(u64, HashOut<V>)>>;

    async fn reset(&self) -> anyhow::Result<()>;
}

#[derive(Clone, Debug)]
pub struct MockDB<V: Leafable> {
    nodes: Arc<RwLock<HashMap<HashOut<V>, Node<V>>>>, // parents hash to node (2 child hashes)
    leaf_hashes: Arc<RwLock<HashMap<u64, HashOut<V>>>>, // position to leaf hash
}

impl<V: Leafable> MockDB<V> {
    pub fn new() -> Self {
        MockDB {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            leaf_hashes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait(?Send)]
impl<V: Leafable> NodeDB<V> for MockDB<V> {
    async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> anyhow::Result<()> {
        self.nodes.write().await.insert(parent_hash, node);
        Ok(())
    }

    async fn get(&self, parent_hash: HashOut<V>) -> anyhow::Result<Option<Node<V>>> {
        Ok(self.nodes.read().await.get(&parent_hash).cloned())
    }

    async fn insert_leaf_hash(&self, position: u64, leaf_hash: HashOut<V>) -> anyhow::Result<()> {
        self.leaf_hashes.write().await.insert(position, leaf_hash);
        Ok(())
    }

    async fn get_all_leaf_hashes(&self) -> anyhow::Result<Vec<(u64, HashOut<V>)>> {
        Ok(self
            .leaf_hashes
            .read()
            .await
            .iter()
            .map(|(position, leaf_hash)| (*position, *leaf_hash))
            .collect())
    }

    async fn reset(&self) -> anyhow::Result<()> {
        self.nodes.write().await.clear();
        self.leaf_hashes.write().await.clear();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RealDB<V: Leafable> {
    pool: Pool<Postgres>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Leafable> RealDB<V> {
    pub async fn new(db_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(db_url)
            .await?;
        Ok(RealDB {
            pool,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[async_trait(?Send)]
impl<V: Leafable> NodeDB<V> for RealDB<V> {
    async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO hash_nodes (parent_hash, left_hash, right_hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (parent_hash) DO NOTHING
            "#,
            bincode::serialize(&parent_hash)? as _,
            bincode::serialize(&node.left_hash)? as _,
            bincode::serialize(&node.right_hash)? as _
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, parent_hash: HashOut<V>) -> anyhow::Result<Option<Node<V>>> {
        let row = sqlx::query!(
            r#"
            SELECT left_hash, right_hash
            FROM hash_nodes
            WHERE parent_hash = $1
            "#,
            bincode::serialize(&parent_hash)? as _
        )
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => Ok(Some(Node {
                left_hash: bincode::deserialize(&row.left_hash)?,
                right_hash: bincode::deserialize(&row.right_hash)?,
            })),
            None => Ok(None),
        }
    }

    async fn insert_leaf_hash(&self, position: u64, leaf_hash: HashOut<V>) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO current_leaf_hashes (position, leaf_hash)
            VALUES ($1, $2)
            ON CONFLICT (position) DO NOTHING
            "#,
            position as i64,
            bincode::serialize(&leaf_hash)? as _
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_all_leaf_hashes(&self) -> anyhow::Result<Vec<(u64, HashOut<V>)>> {
        let rows = sqlx::query!(
            r#"
            SELECT position, leaf_hash
            FROM current_leaf_hashes
            ORDER BY position
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        let mut leaf_hashes = Vec::new();
        for row in rows {
            leaf_hashes.push((row.position as u64, bincode::deserialize(&row.leaf_hash)?));
        }
        Ok(leaf_hashes)
    }

    async fn reset(&self) -> anyhow::Result<()> {
        sqlx::query!("TRUNCATE hash_nodes")
            .execute(&self.pool)
            .await?;

        sqlx::query!("TRUNCATE current_leaf_hashes")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
