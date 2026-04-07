//! Storage layer -- PostgreSQL repositories and Redis/Valkey client.

pub mod postgres;
pub mod redis;
pub mod repositories;

use std::fmt;

/// Errors that can occur in the storage layer.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("{entity} not found by {field} = {value}")]
    NotFound {
        entity: String,
        field: String,
        value: String,
    },

    #[error("duplicate {entity}: {detail}")]
    Conflict { entity: String, detail: String },

    #[error("referenced entity does not exist: {detail}")]
    ReferenceError { detail: String },

    #[error("invalid cursor: {0}")]
    InvalidCursor(String),

    #[error("database error: {0}")]
    Database(#[source] sqlx::Error),

    #[error("redis error: {0}")]
    Redis(String),
}

impl StorageError {
    pub fn not_found(entity: &str, field: &str, value: impl fmt::Display) -> Self {
        Self::NotFound {
            entity: entity.to_owned(),
            field: field.to_owned(),
            value: value.to_string(),
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            if let Some(code) = db_err.code() {
                match code.as_ref() {
                    "23505" => {
                        // Try to extract table name for better error context.
                        let entity = db_err.table().unwrap_or("unknown").to_owned();
                        return Self::Conflict {
                            entity,
                            detail: db_err.message().to_owned(),
                        };
                    }
                    "23503" => {
                        return Self::ReferenceError {
                            detail: db_err.message().to_owned(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Self::Database(err)
    }
}
