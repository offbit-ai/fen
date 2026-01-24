//! gRPC client for connecting to remote shards.

use crate::proto::{
    shard_service_client::ShardServiceClient, AbortRequest, CommitRequest, GetDocumentRequest,
    HealthCheckRequest, PrepareRequest, ShardInfoRequest, StoreDocumentRequest,
    VectorSearchRequest,
};
use async_trait::async_trait;
use fen_cluster::{RemoteShardClient, ShardError};
use fen_core::domain::{NodeId, ShardId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tonic::transport::{Channel, Endpoint};
use tracing::info;

/// Errors that can occur with the shard client.
#[derive(Debug, Error)]
pub enum ShardClientError {
    /// Connection failed.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Request timed out.
    #[error("Request timed out")]
    Timeout,

    /// gRPC error.
    #[error("gRPC error: {0}")]
    GrpcError(#[from] tonic::Status),

    /// Transport error.
    #[error("Transport error: {0}")]
    TransportError(#[from] tonic::transport::Error),

    /// Not connected.
    #[error("Not connected to remote shard")]
    NotConnected,

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl From<ShardClientError> for ShardError {
    fn from(err: ShardClientError) -> Self {
        ShardError::ConnectionError(err.to_string())
    }
}

/// Configuration for the shard client.
#[derive(Debug, Clone)]
pub struct ShardClientConfig {
    /// Remote node address (e.g., "http://shard-1:9000").
    pub address: String,
    /// Remote node ID.
    pub node_id: NodeId,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Request timeout.
    pub request_timeout: Duration,
    /// Keep-alive interval.
    pub keepalive_interval: Duration,
    /// Maximum message size (bytes).
    pub max_message_size: usize,
}

impl Default for ShardClientConfig {
    fn default() -> Self {
        Self {
            address: "http://localhost:9000".to_string(),
            node_id: NodeId::new(),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(10),
            max_message_size: 64 * 1024 * 1024, // 64 MB
        }
    }
}

/// gRPC client for communicating with remote shards.
pub struct ShardClient {
    /// Configuration.
    config: ShardClientConfig,
    /// gRPC client.
    client: ShardServiceClient<Channel>,
    /// Connection status.
    connected: Arc<AtomicBool>,
}

impl ShardClient {
    /// Connect to a remote shard.
    pub async fn connect(config: ShardClientConfig) -> Result<Self, ShardClientError> {
        info!(address = %config.address, "Connecting to remote shard");

        let endpoint = Endpoint::from_shared(config.address.clone())
            .map_err(|e| ShardClientError::ConnectionFailed(e.to_string()))?
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .http2_keep_alive_interval(config.keepalive_interval);

        let channel = endpoint.connect().await?;

        let client = ShardServiceClient::new(channel)
            .max_decoding_message_size(config.max_message_size)
            .max_encoding_message_size(config.max_message_size);

        info!(address = %config.address, "Connected to remote shard");

        Ok(Self {
            config,
            client,
            connected: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Get the remote node's address.
    pub fn address(&self) -> &str {
        &self.config.address
    }

    /// Get the remote node's ID.
    pub fn node_id(&self) -> &NodeId {
        &self.config.node_id
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Store a document on the remote shard.
    pub async fn store_document(
        &mut self,
        request: StoreDocumentRequest,
    ) -> Result<crate::proto::StoreDocumentResponse, ShardClientError> {
        let response = self.client.store_document(request).await?;
        Ok(response.into_inner())
    }

    /// Get a document from the remote shard.
    pub async fn get_document(
        &mut self,
        request: GetDocumentRequest,
    ) -> Result<crate::proto::GetDocumentResponse, ShardClientError> {
        let response = self.client.get_document(request).await?;
        Ok(response.into_inner())
    }

    /// Perform vector search on the remote shard.
    pub async fn vector_search(
        &mut self,
        request: VectorSearchRequest,
    ) -> Result<crate::proto::VectorSearchResponse, ShardClientError> {
        let response = self.client.vector_search(request).await?;
        Ok(response.into_inner())
    }

    /// Prepare a transaction (2PC phase 1).
    pub async fn prepare(
        &mut self,
        request: PrepareRequest,
    ) -> Result<crate::proto::PrepareResponse, ShardClientError> {
        let response = self.client.prepare(request).await?;
        Ok(response.into_inner())
    }

    /// Commit a transaction (2PC phase 2).
    pub async fn commit(
        &mut self,
        request: CommitRequest,
    ) -> Result<crate::proto::CommitResponse, ShardClientError> {
        let response = self.client.commit(request).await?;
        Ok(response.into_inner())
    }

    /// Abort a transaction.
    pub async fn abort(
        &mut self,
        request: AbortRequest,
    ) -> Result<crate::proto::AbortResponse, ShardClientError> {
        let response = self.client.abort(request).await?;
        Ok(response.into_inner())
    }

    /// Perform a health check.
    pub async fn health_check_rpc(
        &mut self,
    ) -> Result<crate::proto::HealthCheckResponse, ShardClientError> {
        let request = HealthCheckRequest {
            include_details: true,
        };
        let response = self.client.health_check(request).await?;
        Ok(response.into_inner())
    }

    /// Get shard information.
    pub async fn get_shard_info(
        &mut self,
        shard_id: ShardId,
    ) -> Result<crate::proto::ShardInfoResponse, ShardClientError> {
        let request = ShardInfoRequest {
            shard_id: Some(crate::proto::ShardId { id: shard_id.0 }),
        };
        let response = self.client.get_shard_info(request).await?;
        Ok(response.into_inner())
    }
}

/// Wrapper to implement RemoteShardClient trait.
pub struct ShardClientWrapper {
    node_id: NodeId,
    address: String,
    client: tokio::sync::Mutex<ShardClient>,
}

impl ShardClientWrapper {
    /// Create a new wrapper from a connected client.
    pub fn new(client: ShardClient) -> Self {
        Self {
            node_id: client.config.node_id.clone(),
            address: client.config.address.clone(),
            client: tokio::sync::Mutex::new(client),
        }
    }

    /// Get access to the underlying client.
    pub async fn client(&self) -> tokio::sync::MutexGuard<'_, ShardClient> {
        self.client.lock().await
    }
}

#[async_trait]
impl RemoteShardClient for ShardClientWrapper {
    fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    fn address(&self) -> &str {
        &self.address
    }

    async fn health_check(&self) -> Result<(), ShardError> {
        let mut client = self.client.lock().await;
        match client.health_check_rpc().await {
            Ok(response) => {
                if response.healthy {
                    Ok(())
                } else {
                    Err(ShardError::ConnectionError(format!(
                        "Unhealthy: {}",
                        response.status
                    )))
                }
            }
            Err(e) => Err(ShardError::ConnectionError(e.to_string())),
        }
    }

    fn is_connected(&self) -> bool {
        // Note: This is a best-effort check without locking
        true // The actual connection check would require async
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = ShardClientConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
    }
}
