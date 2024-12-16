use intmax2_zkp::utils::{leafable::Leafable, leafable_hasher::LeafableHasher};
use sqlx::{Pool, Postgres, Row};
use std::marker::PhantomData;

// Node structure remains similar but serialization is needed for database storage
#[derive(Clone, Debug)]
pub struct Node<V: Leafable> {
    pub left: <V::LeafableHasher as LeafableHasher>::HashOut,
    pub right: <V::LeafableHasher as LeafableHasher>::HashOut,
    _phantom: PhantomData<V>, // Required for generic type parameter
}

// Database wrapper using SQLx with PostgreSQL
#[derive(Debug)]
pub struct SqlxDB<V: Leafable> {
    pool: Pool<Postgres>,
    _phantom: PhantomData<V>,
}

impl<V: Leafable> SqlxDB<V> {
    // Create a new database connection pool
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = Pool::<Postgres>::connect(database_url).await?;

        // Create the table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS merkle_nodes (
                hash_key BYTEA PRIMARY KEY,
                left_hash BYTEA NOT NULL,
                right_hash BYTEA NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(SqlxDB {
            pool,
            _phantom: PhantomData,
        })
    }

    // Insert a new node into the database
    pub async fn insert(
        &self,
        parent_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
        node: Node<V>,
    ) -> Result<(), sqlx::Error> {
        let parent_bytes = bincode::serialize(&parent_hash).unwrap();
        let left_bytes = bincode::serialize(&node.left).unwrap();
        let right_bytes = bincode::serialize(&node.right).unwrap();

        sqlx::query(
            r#"
            INSERT INTO merkle_nodes (hash_key, left_hash, right_hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (hash_key) DO UPDATE
            SET left_hash = $2, right_hash = $3
            "#,
        )
        .bind(parent_bytes)
        .bind(left_bytes)
        .bind(right_bytes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Retrieve a node from the database
    pub async fn get(
        &self,
        parent_hash: <V::LeafableHasher as LeafableHasher>::HashOut,
    ) -> Result<Option<Node<V>>, sqlx::Error> {
        let parent_bytes = bincode::serialize(&parent_hash).unwrap();

        let row = sqlx::query(
            r#"
            SELECT left_hash, right_hash
            FROM merkle_nodes
            WHERE hash_key = $1
            "#,
        )
        .bind(parent_bytes)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let left_bytes: Vec<u8> = row.get(0);
                let right_bytes: Vec<u8> = row.get(1);

                let left = bincode::deserialize(&left_bytes).unwrap();
                let right = bincode::deserialize(&right_bytes).unwrap();

                Ok(Some(Node {
                    left,
                    right,
                    _phantom: PhantomData,
                }))
            }
            None => Ok(None),
        }
    }
}
