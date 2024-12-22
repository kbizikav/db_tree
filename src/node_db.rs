use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

type HashOut<V> = <<V as Leafable>::LeafableHasher as LeafableHasher>::HashOut;

#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left: HashOut<V>,
    pub right: HashOut<V>,
}

#[derive(Clone, Debug)]
pub struct NodeDB<V: Leafable> {
    pool: Pool<Postgres>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Leafable> NodeDB<V> {
    pub async fn new(db_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(db_url)
            .await?;
        Ok(NodeDB {
            pool,
            _phantom: std::marker::PhantomData,
        })
    }

    pub async fn insert(&self, parent_hash: HashOut<V>, node: Node<V>) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO hash_nodes (parent_hash, left_hash, right_hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (parent_hash) DO NOTHING
            "#,
            bincode::serialize(&parent_hash)? as _,
            bincode::serialize(&node.left)? as _,
            bincode::serialize(&node.right)? as _
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, parent_hash: HashOut<V>) -> anyhow::Result<Option<Node<V>>> {
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
                left: bincode::deserialize(&row.left_hash)?,
                right: bincode::deserialize(&row.right_hash)?,
            })),
            None => Ok(None),
        }
    }

    pub async fn insert_leaf_hash(
        &self,
        position: u64,
        leaf_hash: HashOut<V>,
    ) -> anyhow::Result<()> {
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

    pub async fn get_all_leaf_hashes(&self) -> anyhow::Result<Vec<(u64, HashOut<V>)>> {
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
}
