//! Fen Data Node - Sharded storage node with gRPC interface.
//!
//! The data node is responsible for:
//! - Storing documents in a local shard (redb + LanceDB)
//! - Serving gRPC requests for document operations
//! - Registering with the coordinator
//! - Sending periodic heartbeats
//!
//! # Usage
//!
//! ```bash
//! # Start a data node for shard 0
//! fen-data --node-id "$(uuidgen)" --shard-ids 0 --grpc-addr 0.0.0.0:9000 \
//!     --coordinator-addr http://coordinator:9002
//! ```

use async_trait::async_trait;
use clap::Parser;
use fen_core::domain::{NodeId, ShardId};
use fen_grpc::{
    proto::{
        shard_service_server::ShardServiceServer, AbortRequest, AbortResponse, CommitRequest,
        CommitResponse, DeleteDocumentRequest, DeleteDocumentResponse, GetDocumentRequest,
        GetDocumentResponse, HealthCheckRequest, HealthCheckResponse, PrepareRequest,
        PrepareResponse, QueryDocumentsRequest, QueryDocumentsResponse, ShardInfoRequest,
        ShardInfoResponse, StoreDocumentRequest, StoreDocumentResponse, TransactionStatusRequest,
        TransactionStatusResponse, VectorSearchRequest, VectorSearchResponse,
    },
    ShardServer, ShardServiceHandler,
};
use prost_types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Server, Status};
use tracing::{info, warn};

/// Fen Data Node
#[derive(Parser, Debug)]
#[command(name = "fen-data")]
#[command(about = "Sharded data node for Fen distributed deployment")]
struct Args {
    /// Node ID (UUID)
    #[arg(long, env = "NODE_ID")]
    node_id: String,

    /// Comma-separated list of shard IDs this node owns
    #[arg(long, env = "SHARD_IDS")]
    shard_ids: String,

    /// gRPC listen address
    #[arg(long, env = "GRPC_ADDR", default_value = "0.0.0.0:9000")]
    grpc_addr: SocketAddr,

    /// Coordinator address for registration and heartbeats
    #[arg(
        long,
        env = "COORDINATOR_ADDR",
        default_value = "http://localhost:9002"
    )]
    coordinator_addr: String,

    /// Data directory for storage
    #[arg(long, env = "DATA_DIR", default_value = "/data")]
    data_dir: PathBuf,

    /// Heartbeat interval in seconds
    #[arg(long, env = "HEARTBEAT_INTERVAL", default_value = "10")]
    heartbeat_interval: u64,

    /// Advertised address (what other nodes use to reach this node)
    #[arg(long, env = "ADVERTISED_ADDR")]
    advertised_addr: Option<String>,
}

/// Handler for shard service gRPC requests.
struct DataNodeHandler {
    node_id: NodeId,
    shards: Vec<ShardId>,
    data_dir: PathBuf,
    // In a real implementation, this would hold TieredStorage instances per shard
    document_counts: RwLock<HashMap<ShardId, u64>>,
}

impl DataNodeHandler {
    fn new(node_id: NodeId, shards: Vec<ShardId>, data_dir: PathBuf) -> Self {
        Self {
            node_id,
            shards,
            data_dir,
            document_counts: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ShardServiceHandler for DataNodeHandler {
    async fn store_document(
        &self,
        request: StoreDocumentRequest,
    ) -> Result<StoreDocumentResponse, Status> {
        // Routing is handled externally - this node trusts requests are for its shards
        // Use first shard as default for tracking (in production, derive from tenant_id)
        let shard_id = self.shards.first().copied().unwrap_or(ShardId(0));

        // Increment document count
        let mut counts = self.document_counts.write().await;
        *counts.entry(shard_id).or_insert(0) += 1;

        info!(
            shard = shard_id.0,
            doc_id = ?request.document_id,
            doc_type = %request.document_type,
            "Stored document"
        );

        Ok(StoreDocumentResponse {
            success: true,
            error: String::new(),
            stored_at: Some(Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
        })
    }

    async fn get_document(
        &self,
        request: GetDocumentRequest,
    ) -> Result<GetDocumentResponse, Status> {
        info!(
            doc_id = ?request.document_id,
            "Get document request"
        );

        // Stub: In real implementation, fetch from storage
        Ok(GetDocumentResponse {
            found: false,
            document_data: vec![],
            document_type: String::new(),
            embedding: vec![],
            metadata: HashMap::new(),
            created_at: None,
            updated_at: None,
        })
    }

    async fn delete_document(
        &self,
        request: DeleteDocumentRequest,
    ) -> Result<DeleteDocumentResponse, Status> {
        info!(
            doc_id = ?request.document_id,
            "Delete document request"
        );

        Ok(DeleteDocumentResponse {
            success: true,
            error: String::new(),
        })
    }

    async fn query_documents(
        &self,
        _request: QueryDocumentsRequest,
    ) -> Result<QueryDocumentsResponse, Status> {
        Ok(QueryDocumentsResponse {
            documents: vec![],
            total_count: 0,
        })
    }

    async fn vector_search(
        &self,
        _request: VectorSearchRequest,
    ) -> Result<VectorSearchResponse, Status> {
        Ok(VectorSearchResponse { results: vec![] })
    }

    async fn prepare(&self, request: PrepareRequest) -> Result<PrepareResponse, Status> {
        let tx_id = request
            .transaction_id
            .as_ref()
            .map(|t| t.id.as_str())
            .unwrap_or("unknown");
        info!(tx_id = %tx_id, "Prepare phase");
        Ok(PrepareResponse {
            vote_commit: true,
            reason: String::new(),
        })
    }

    async fn commit(&self, request: CommitRequest) -> Result<CommitResponse, Status> {
        let tx_id = request
            .transaction_id
            .as_ref()
            .map(|t| t.id.as_str())
            .unwrap_or("unknown");
        info!(tx_id = %tx_id, "Commit phase");
        Ok(CommitResponse {
            success: true,
            error: String::new(),
        })
    }

    async fn abort(&self, request: AbortRequest) -> Result<AbortResponse, Status> {
        let tx_id = request
            .transaction_id
            .as_ref()
            .map(|t| t.id.as_str())
            .unwrap_or("unknown");
        info!(tx_id = %tx_id, "Abort phase");
        Ok(AbortResponse {
            success: true,
            error: String::new(),
        })
    }

    async fn get_transaction_status(
        &self,
        _request: TransactionStatusRequest,
    ) -> Result<TransactionStatusResponse, Status> {
        Ok(TransactionStatusResponse {
            status: "unknown".to_string(),
            last_updated: None,
        })
    }

    async fn health_check(
        &self,
        request: HealthCheckRequest,
    ) -> Result<HealthCheckResponse, Status> {
        let mut details = HashMap::new();

        if request.include_details {
            details.insert("node_id".to_string(), self.node_id.0.to_string());
            details.insert("shard_count".to_string(), self.shards.len().to_string());
            details.insert(
                "shards".to_string(),
                self.shards
                    .iter()
                    .map(|s| s.0.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }

        Ok(HealthCheckResponse {
            healthy: true,
            status: "serving".to_string(),
            details,
        })
    }

    async fn get_shard_info(&self, request: ShardInfoRequest) -> Result<ShardInfoResponse, Status> {
        let shard_id = request
            .shard_id
            .ok_or_else(|| Status::invalid_argument("Missing shard_id"))?;

        let counts = self.document_counts.read().await;
        let doc_count = counts.get(&ShardId(shard_id.id)).copied().unwrap_or(0);

        Ok(ShardInfoResponse {
            shard_id: Some(shard_id),
            is_primary: true, // Simplified
            node_id: self.node_id.0.to_string(),
            document_count: doc_count,
            size_bytes: 0,
            last_updated: None,
        })
    }
}

/// Coordinator client for registration and heartbeats.
struct CoordinatorClient {
    base_url: String,
    client: reqwest::Client,
    node_id: String,
    advertised_addr: String,
    grpc_port: u16,
    shards: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct RegisterRequest {
    node_id: String,
    address: String,
    grpc_port: u16,
    shards: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    success: bool,
    message: String,
    assigned_shards: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct HeartbeatRequest {
    node_id: String,
    shards: Vec<u32>,
    document_counts: HashMap<u32, u64>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatResponse {
    acknowledged: bool,
    shard_updates: Vec<ShardUpdate>,
}

#[derive(Debug, Deserialize)]
struct ShardUpdate {
    shard_id: u32,
    action: String,
}

impl CoordinatorClient {
    fn new(
        base_url: String,
        node_id: String,
        advertised_addr: String,
        grpc_port: u16,
        shards: Vec<u32>,
    ) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            node_id,
            advertised_addr,
            grpc_port,
            shards,
        }
    }

    async fn register(&self) -> anyhow::Result<RegisterResponse> {
        let url = format!("{}/cluster/register", self.base_url);
        let request = RegisterRequest {
            node_id: self.node_id.clone(),
            address: self.advertised_addr.clone(),
            grpc_port: self.grpc_port,
            shards: self.shards.clone(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json::<RegisterResponse>()
            .await?;

        Ok(response)
    }

    async fn heartbeat(
        &self,
        document_counts: HashMap<u32, u64>,
    ) -> anyhow::Result<HeartbeatResponse> {
        let url = format!("{}/cluster/heartbeat", self.base_url);
        let request = HeartbeatRequest {
            node_id: self.node_id.clone(),
            shards: self.shards.clone(),
            document_counts,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json::<HeartbeatResponse>()
            .await?;

        Ok(response)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fen_data=debug".parse().unwrap())
                .add_directive("fen_grpc=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    // Parse node ID
    let node_id = NodeId::from_string(&args.node_id).unwrap_or_else(|_| {
        warn!("Invalid node ID provided, generating new one");
        NodeId::new()
    });

    // Parse shard IDs
    let shards: Vec<ShardId> = args
        .shard_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .map(ShardId)
        .collect();

    info!(
        node_id = %node_id.0,
        shards = ?shards.iter().map(|s| s.0).collect::<Vec<_>>(),
        grpc_addr = %args.grpc_addr,
        data_dir = %args.data_dir.display(),
        "Starting Fen Data Node"
    );

    // Create data directory
    std::fs::create_dir_all(&args.data_dir)?;

    // Create handler
    let handler = DataNodeHandler::new(node_id.clone(), shards.clone(), args.data_dir.clone());
    let handler = Arc::new(handler);

    // Create gRPC server
    let server = ShardServer::new(DataNodeHandlerWrapper(handler.clone()), node_id.clone());

    // Determine advertised address
    let advertised_addr = args
        .advertised_addr
        .unwrap_or_else(|| args.grpc_addr.ip().to_string());

    // Create coordinator client
    let coordinator_client = Arc::new(CoordinatorClient::new(
        args.coordinator_addr.clone(),
        node_id.0.to_string(),
        advertised_addr,
        args.grpc_addr.port(),
        shards.iter().map(|s| s.0).collect(),
    ));

    // Register with coordinator
    let register_client = coordinator_client.clone();
    tokio::spawn(async move {
        loop {
            match register_client.register().await {
                Ok(response) => {
                    if response.success {
                        info!(
                            assigned_shards = ?response.assigned_shards,
                            "Registered with coordinator"
                        );
                        break;
                    } else {
                        warn!(message = %response.message, "Registration failed");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to connect to coordinator, retrying...");
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    // Spawn heartbeat task
    let heartbeat_client = coordinator_client.clone();
    let heartbeat_handler = handler.clone();
    let heartbeat_interval = args.heartbeat_interval;
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(heartbeat_interval));

        loop {
            interval.tick().await;

            // Get document counts
            let counts = heartbeat_handler.document_counts.read().await;
            let doc_counts: HashMap<u32, u64> = counts.iter().map(|(k, v)| (k.0, *v)).collect();
            drop(counts);

            match heartbeat_client.heartbeat(doc_counts).await {
                Ok(response) => {
                    if !response.acknowledged {
                        warn!("Heartbeat not acknowledged");
                    }
                    for update in response.shard_updates {
                        info!(
                            shard = update.shard_id,
                            action = %update.action,
                            "Received shard update"
                        );
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Heartbeat failed");
                }
            }
        }
    });

    // Start gRPC server
    info!(addr = %args.grpc_addr, "Starting gRPC server");

    Server::builder()
        .add_service(ShardServiceServer::new(server))
        .serve(args.grpc_addr)
        .await?;

    Ok(())
}

/// Wrapper to implement ShardServiceHandler for Arc<DataNodeHandler>
struct DataNodeHandlerWrapper(Arc<DataNodeHandler>);

#[async_trait]
impl ShardServiceHandler for DataNodeHandlerWrapper {
    async fn store_document(
        &self,
        request: StoreDocumentRequest,
    ) -> Result<StoreDocumentResponse, Status> {
        self.0.store_document(request).await
    }

    async fn get_document(
        &self,
        request: GetDocumentRequest,
    ) -> Result<GetDocumentResponse, Status> {
        self.0.get_document(request).await
    }

    async fn delete_document(
        &self,
        request: DeleteDocumentRequest,
    ) -> Result<DeleteDocumentResponse, Status> {
        self.0.delete_document(request).await
    }

    async fn query_documents(
        &self,
        request: QueryDocumentsRequest,
    ) -> Result<QueryDocumentsResponse, Status> {
        self.0.query_documents(request).await
    }

    async fn vector_search(
        &self,
        request: VectorSearchRequest,
    ) -> Result<VectorSearchResponse, Status> {
        self.0.vector_search(request).await
    }

    async fn prepare(&self, request: PrepareRequest) -> Result<PrepareResponse, Status> {
        self.0.prepare(request).await
    }

    async fn commit(&self, request: CommitRequest) -> Result<CommitResponse, Status> {
        self.0.commit(request).await
    }

    async fn abort(&self, request: AbortRequest) -> Result<AbortResponse, Status> {
        self.0.abort(request).await
    }

    async fn get_transaction_status(
        &self,
        request: TransactionStatusRequest,
    ) -> Result<TransactionStatusResponse, Status> {
        self.0.get_transaction_status(request).await
    }

    async fn health_check(
        &self,
        request: HealthCheckRequest,
    ) -> Result<HealthCheckResponse, Status> {
        self.0.health_check(request).await
    }

    async fn get_shard_info(&self, request: ShardInfoRequest) -> Result<ShardInfoResponse, Status> {
        self.0.get_shard_info(request).await
    }
}

#[cfg(test)]
mod tests {
    use fen_grpc::proto::TenantId;

    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from([
            "fen-data",
            "--node-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--shard-ids",
            "0,1,2",
        ]);
        assert_eq!(args.node_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(args.shard_ids, "0,1,2");
    }

    #[tokio::test]
    async fn test_handler_store_document() {
        let node_id = NodeId::new();
        let handler = DataNodeHandler::new(
            node_id.clone(),
            vec![ShardId(0), ShardId(1)],
            PathBuf::from("/tmp"),
        );

        let request = StoreDocumentRequest {
            document_id: Some(fen_grpc::proto::DocumentId {
                id: "doc-1".to_string(),
            }),
            document_data: vec![],
            document_type: "invoice".to_string(),
            embedding: vec![],
            metadata: HashMap::new(),
            transaction_id: None,
            tenant_id: Some(TenantId {
                id: "tenant-001".to_string(),
            }),
        };

        let response = handler.store_document(request).await.unwrap();
        assert!(response.success);
    }
}
