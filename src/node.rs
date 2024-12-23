use crate::error::NodeDBError;
use async_trait::async_trait;
use hashbrown::HashMap;
use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;

pub type NodeDBResult<T> = Result<T, NodeDBError>;
type Hasher<V> = <V as Leafable>::LeafableHasher;
type HashOut<V> = <Hasher<V> as LeafableHasher>::HashOut;

#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left_hash: HashOut<V>,
    pub right_hash: HashOut<V>,
}

#[async_trait(?Send)]
pub trait NodeDB<V: Leafable>: std::fmt::Debug + Clone {
    async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> NodeDBResult<()>;
    async fn get(&self, parent_hash: HashOut<V>) -> NodeDBResult<Option<Node<V>>>;
    async fn insert_leaf_hash(&self, position: u64, leaf_hash: HashOut<V>) -> NodeDBResult<()>;
    async fn get_leaf_hash(&self, position: u64) -> NodeDBResult<Option<HashOut<V>>>;
    async fn get_all_leaf_hashes(&self) -> NodeDBResult<Vec<(u64, HashOut<V>)>>;
    async fn reset(&self) -> NodeDBResult<()>;
}

#[derive(Clone, Debug)]
pub struct MockNodeDB<V: Leafable> {
    nodes: Arc<RwLock<HashMap<HashOut<V>, Node<V>>>>,
    leaf_hashes: Arc<RwLock<HashMap<u64, HashOut<V>>>>,
}

impl<V: Leafable> MockNodeDB<V> {
    pub fn new() -> Self {
        MockNodeDB {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            leaf_hashes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait(?Send)]
impl<V: Leafable> NodeDB<V> for MockNodeDB<V> {
    async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> NodeDBResult<()> {
        self.nodes.write().await.insert(parent_hash, node);
        Ok(())
    }

    async fn get(&self, parent_hash: HashOut<V>) -> NodeDBResult<Option<Node<V>>> {
        Ok(self.nodes.read().await.get(&parent_hash).cloned())
    }

    async fn insert_leaf_hash(&self, position: u64, leaf_hash: HashOut<V>) -> NodeDBResult<()> {
        self.leaf_hashes.write().await.insert(position, leaf_hash);
        Ok(())
    }

    async fn get_leaf_hash(&self, position: u64) -> NodeDBResult<Option<HashOut<V>>> {
        Ok(self.leaf_hashes.read().await.get(&position).cloned())
    }

    async fn get_all_leaf_hashes(&self) -> NodeDBResult<Vec<(u64, HashOut<V>)>> {
        Ok(self
            .leaf_hashes
            .read()
            .await
            .iter()
            .map(|(position, leaf_hash)| (*position, *leaf_hash))
            .collect())
    }

    async fn reset(&self) -> NodeDBResult<()> {
        self.nodes.write().await.clear();
        self.leaf_hashes.write().await.clear();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SqlNodeDB<V: Leafable> {
    pool: Pool<Postgres>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Leafable> SqlNodeDB<V> {
    pub async fn new(db_url: &str) -> NodeDBResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(db_url)
            .await?;
        Ok(SqlNodeDB {
            pool,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[async_trait(?Send)]
impl<V: Leafable> NodeDB<V> for SqlNodeDB<V> {
    async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> NodeDBResult<()> {
        let serialized_parent = bincode::serialize(&parent_hash)?;
        let serialized_left = bincode::serialize(&node.left_hash)?;
        let serialized_right = bincode::serialize(&node.right_hash)?;

        sqlx::query!(
            r#"
            INSERT INTO hash_nodes (parent_hash, left_hash, right_hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (parent_hash) DO NOTHING
            "#,
            serialized_parent as _,
            serialized_left as _,
            serialized_right as _
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, parent_hash: HashOut<V>) -> NodeDBResult<Option<Node<V>>> {
        let serialized_hash = bincode::serialize(&parent_hash)?;

        let row = sqlx::query!(
            r#"
            SELECT left_hash, right_hash
            FROM hash_nodes
            WHERE parent_hash = $1
            "#,
            serialized_hash as _
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let left_hash = bincode::deserialize(&row.left_hash)?;
                let right_hash = bincode::deserialize(&row.right_hash)?;

                Ok(Some(Node {
                    left_hash,
                    right_hash,
                }))
            }
            None => Ok(None),
        }
    }

    async fn insert_leaf_hash(&self, position: u64, leaf_hash: HashOut<V>) -> NodeDBResult<()> {
        let serialized_hash = bincode::serialize(&leaf_hash)?;

        sqlx::query!(
            r#"
            INSERT INTO current_leaf_hashes (position, leaf_hash)
            VALUES ($1, $2)
            ON CONFLICT (position) DO NOTHING
            "#,
            position as i64,
            serialized_hash as _
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_leaf_hash(&self, position: u64) -> NodeDBResult<Option<HashOut<V>>> {
        let row = sqlx::query!(
            r#"
            SELECT leaf_hash
            FROM current_leaf_hashes
            WHERE position = $1
            "#,
            position as i64
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let hash = bincode::deserialize(&row.leaf_hash)?;
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }

    #[instrument(target = "get_all_leaf_hashes", level = "trace")]
    async fn get_all_leaf_hashes(&self) -> NodeDBResult<Vec<(u64, HashOut<V>)>> {
        let time = std::time::Instant::now();
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
            let hash = bincode::deserialize(&row.leaf_hash)?;
            leaf_hashes.push((row.position as u64, hash));
        }

        tracing::info!("get_all_leaf_hashes took {:?}", time.elapsed());
        Ok(leaf_hashes)
    }

    async fn reset(&self) -> NodeDBResult<()> {
        sqlx::query!("TRUNCATE hash_nodes")
            .execute(&self.pool)
            .await?;

        sqlx::query!("TRUNCATE current_leaf_hashes")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
