use serde::Deserialize;

pub mod merkle_tree;
pub mod node_db;

#[derive(Deserialize)]
pub struct EnvVar {
    pub database_url: String,
}
