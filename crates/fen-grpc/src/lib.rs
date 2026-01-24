//! gRPC service definitions and implementations for Fen cluster communication.
//!
//! This crate provides:
//! - Protobuf-generated types for shard service communication
//! - Client implementation for connecting to remote shards
//! - Server implementation for handling incoming requests
//! - Connection pooling for efficient inter-node communication
//!
//! # Example
//!
//! ```ignore
//! use fen_grpc::{ShardClient, ShardClientConfig};
//!
//! // Create a client to connect to a remote shard
//! let config = ShardClientConfig {
//!     address: "http://shard-1:9000".to_string(),
//!     ..Default::default()
//! };
//! let client = ShardClient::connect(config).await?;
//!
//! // Use the client
//! let response = client.health_check().await?;
//! ```

pub mod client;
pub mod server;
pub mod pool;

// Re-export generated protobuf types
pub mod proto {
    tonic::include_proto!("fen.shard.v1");
}

pub use client::{ShardClient, ShardClientConfig, ShardClientError};
pub use pool::{ConnectionPool, ConnectionPoolConfig};
pub use server::{ShardServer, ShardServiceHandler};

// Re-export commonly used proto types
pub use proto::{
    shard_service_client::ShardServiceClient,
    shard_service_server::{ShardService, ShardServiceServer},
    CommitRequest, CommitResponse, GetDocumentRequest, GetDocumentResponse,
    HealthCheckRequest, HealthCheckResponse, PrepareRequest, PrepareResponse,
    StoreDocumentRequest, StoreDocumentResponse,
};
