use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left: <V::LeafableHasher as LeafableHasher>::HashOut,
    pub right: <V::LeafableHasher as LeafableHasher>::HashOut,
}

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

    pub async fn insert(
        &self,
        parent_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
        node: Node<V>,
    ) -> anyhow::Result<()> {
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

    pub async fn get(
        &self,
        parent_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) -> anyhow::Result<Option<Node<V>>> {
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
}
