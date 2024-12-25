// CREATE TABLE IF NOT EXISTS hash_nodes (
//     timestamp_value bigint NOT NULL,
//     tag int NOT NULL,
//     bit_path bytea NOT NULL,
//     hash_value bytea NOT NULL,
//     PRIMARY KEY (timestamp_value, tag, bit_path)
// );

// CREATE TABLE IF NOT EXISTS leaves (
//     timestamp_value bigint NOT NULL,
//     tag int NOT NULL,
//     position bigint NOT NULL,
//     leaf_hash bytea NOT NULL,
//     leaf bytea NOT NULL,
//     PRIMARY KEY (timestamp_value, tag, position)
// );

// CREATE TABLE IF NOT EXISTS leaves_len (
//     timestamp_value bigint NOT NULL,
//     tag int NOT NULL,
//     len int NOT NULL,
//     PRIMARY KEY (timestamp_value, tag)
// );

use intmax2_zkp::utils::leafable_hasher::LeafableHasher;
use intmax2_zkp::utils::trees::merkle_tree::MerkleProof;
use intmax2_zkp::{common::tx, utils::leafable::Leafable};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{Pool, Postgres};

use crate::utils::bit_path::BitPath;

use super::{error::MerkleTreeError, HashOut, Hasher, MTResult};

#[derive(Clone, Debug)]
pub struct SqlMerkleTree<V: Leafable + Serialize + DeserializeOwned> {
    tag: u32, // tag is used to distinguish between different trees in the same database
    height: usize,
    zero_hashes: Vec<HashOut<V>>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Leafable + Serialize + DeserializeOwned> SqlMerkleTree<V> {
    pub fn new(tag: u32, height: usize) -> Self {
        let mut zero_hashes = vec![];
        let mut h = V::empty_leaf().hash();
        zero_hashes.push(h.clone());
        for _ in 0..height {
            let new_h = Hasher::<V>::two_to_one(h, h);
            zero_hashes.push(new_h);
            h = new_h;
        }
        zero_hashes.reverse();
        SqlMerkleTree {
            tag,
            height,
            zero_hashes,
            _phantom: std::marker::PhantomData,
        }
    }

    async fn save_node(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        bit_path: BitPath,
        hash: HashOut<V>,
    ) -> MTResult<()> {
        let bit_path = bincode::serialize(&bit_path).unwrap();
        let hash = bincode::serialize(&hash).unwrap();
        sqlx::query!(
            r#"
            INSERT INTO hash_nodes (timestamp_value, tag, bit_path, hash_value)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (timestamp_value, tag, bit_path)
            DO UPDATE SET hash_value = $4
            "#,
            timestamp as i64,
            self.tag as i32,
            bit_path,
            hash,
        )
        .execute(tx.as_mut())
        .await?;
        Ok(())
    }

    async fn get_node_hash_at_timestamp(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        bit_path: BitPath,
    ) -> MTResult<HashOut<V>> {
        let bit_path = bincode::serialize(&bit_path).unwrap();
        let record = sqlx::query!(
            r#"
        SELECT hash_value 
        FROM hash_nodes 
        WHERE bit_path = $1 
          AND timestamp_value <= $2 
          AND tag = $3 
        ORDER BY timestamp_value DESC 
        LIMIT 1
        "#,
            bit_path,
            timestamp as i64,
            self.tag as i32
        )
        .fetch_optional(tx.as_mut())
        .await?;

        match record {
            Some(row) => {
                let hash = bincode::deserialize(&row.hash_value).unwrap();
                Ok(hash)
            }
            None => Ok(self.zero_hashes[bit_path.len()]),
        }
    }

    async fn save_leaf(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        position: u64,
        leaf: V,
    ) -> super::MTResult<()> {
        let leaf_hash = bincode::serialize(&leaf.hash()).unwrap();
        let leaf = bincode::serialize(&leaf).unwrap();

        let current_len = self.get_num_leaves_at_timestamp(tx, timestamp).await?;
        let next_len = ((position + 1) as usize).max(current_len);

        sqlx::query!(
            r#"
            INSERT INTO leaves (timestamp_value, tag, position, leaf_hash, leaf)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (timestamp_value, tag, position)
            DO UPDATE SET leaf_hash = $4, leaf = $5
            "#,
            timestamp as i64,
            self.tag as i32,
            position as i64,
            leaf_hash,
            leaf,
        )
        .execute(tx.as_mut())
        .await?;
        sqlx::query!(
            r#"
            INSERT INTO leaves_len (timestamp_value, tag, len)
            VALUES ($1, $2, $3)
            ON CONFLICT (timestamp_value, tag)
            DO UPDATE SET len = $3
            "#,
            timestamp as i64,
            self.tag as i32,
            next_len as i32,
        )
        .execute(tx.as_mut())
        .await?;

        Ok(())
    }

    pub async fn get_leaf_at_timestamp(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        position: u64,
    ) -> super::MTResult<V> {
        let record = sqlx::query!(
            r#"
        SELECT leaf 
        FROM leaves 
        WHERE position = $1 
          AND timestamp_value <= $2 
          AND tag = $3 
        ORDER BY timestamp_value DESC 
        LIMIT 1
        "#,
            position as i64,
            timestamp as i64,
            self.tag as i32
        )
        .fetch_optional(tx.as_mut())
        .await?;

        match record {
            Some(row) => {
                let leaf = bincode::deserialize(&row.leaf).unwrap();
                Ok(leaf)
            }
            None => Ok(V::empty_leaf()),
        }
    }

    pub async fn get_leaves_at_timestamp(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
    ) -> MTResult<Vec<(u64, V)>> {
        let records = sqlx::query!(
            r#"
            WITH RankedLeaves AS (
                SELECT *,
                    ROW_NUMBER() OVER (
                        PARTITION BY position 
                        ORDER BY timestamp_value DESC
                    ) as rn
                FROM leaves
                WHERE timestamp_value <= $1
            )
            SELECT 
                timestamp_value,
                tag,
                position,
                leaf_hash,
                leaf
            FROM RankedLeaves
            WHERE rn = 1
            ORDER BY position
            "#,
            timestamp as i64
        )
        .fetch_all(tx.as_mut())
        .await?;

        let mut leaves = vec![];
        for record in records {
            let position = record.position as u64;
            let leaf: V = bincode::deserialize(&record.leaf).unwrap();
            leaves.push((position, leaf));
        }

        Ok(leaves)
    }

    pub async fn get_num_leaves_at_timestamp(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
    ) -> super::MTResult<usize> {
        let record = sqlx::query!(
            r#"
            SELECT len
            FROM leaves_len
            WHERE timestamp_value <= $1
              AND tag = $2
            ORDER BY timestamp_value DESC
            LIMIT 1
            "#,
            timestamp as i64,
            self.tag as i32
        )
        .fetch_optional(tx.as_mut())
        .await?;

        match record {
            Some(row) => {
                let len = row.len as usize;
                Ok(len)
            }
            None => Ok(0),
        }
    }

    async fn get_latest_timestamp(&self, tx: &mut sqlx::Transaction<'_, Postgres>) -> u64 {
        let record = sqlx::query!(
            r#"
            SELECT timestamp_value
            FROM leaves_len
            WHERE tag = $1
            ORDER BY timestamp_value DESC
            LIMIT 1
            "#,
            self.tag as i32
        )
        .fetch_optional(tx.as_mut())
        .await
        .unwrap();

        match record {
            Some(row) => row.timestamp_value as u64,
            None => 0,
        }
    }

    async fn get_sibling_hash(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        path: BitPath,
    ) -> MTResult<HashOut<V>> {
        if path.is_empty() {
            return Err(MerkleTreeError::WrongPathLength(0));
        }
        let sibling_path = path.sibling();
        let sibling_hash = self
            .get_node_hash_at_timestamp(tx, timestamp, sibling_path)
            .await?;
        Ok(sibling_hash)
    }

    pub async fn get_root(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
    ) -> MTResult<HashOut<V>> {
        self.get_node_hash_at_timestamp(tx, timestamp, BitPath::default())
            .await
    }

    pub async fn update_leaf(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        index: u64,
        leaf: V,
    ) -> super::MTResult<()> {
        let mut path = BitPath::new(self.height as u32, index);
        path.reverse();
        let mut h = leaf.hash();

        self.save_leaf(tx, timestamp, index, leaf).await?;
        while !path.is_empty() {
            let sibling = self.get_sibling_hash(tx, timestamp, path).await?;
            dbg!(&sibling);
            let b = path.pop().unwrap(); // safe to unwrap
            let new_h = if b {
                Hasher::<V>::two_to_one(sibling, h)
            } else {
                Hasher::<V>::two_to_one(h, sibling)
            };
            self.save_node(tx, timestamp, path, new_h).await?;
            h = new_h;
        }
        Ok(())
    }

    pub async fn prove(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        timestamp: u64,
        index: u64,
    ) -> MTResult<MerkleProof<V>> {
        let mut path = BitPath::new(self.height as u32, index);
        path.reverse(); // path is big endian
        let mut siblings = vec![];
        while !path.is_empty() {
            siblings.push(self.get_sibling_hash(tx, timestamp, path).await?);
            path.pop();
        }
        Ok(MerkleProof { siblings })
    }

    pub async fn reset(&self, tx: &mut sqlx::Transaction<'_, Postgres>) -> MTResult<()> {
        sqlx::query!(
            r#"
            DELETE FROM hash_nodes
            WHERE tag = $1
            "#,
            self.tag as i32
        )
        .execute(tx.as_mut())
        .await?;

        sqlx::query!(
            r#"
            DELETE FROM leaves
            WHERE tag = $1
            "#,
            self.tag as i32
        )
        .execute(tx.as_mut())
        .await?;

        sqlx::query!(
            r#"
            DELETE FROM leaves_len
            WHERE tag = $1
            "#,
            self.tag as i32
        )
        .execute(tx.as_mut())
        .await?;

        Ok(())
    }
}
