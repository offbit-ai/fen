//! gRPC server for handling shard service requests.

use crate::proto::{
    shard_service_server::ShardService, AbortRequest, AbortResponse, CommitRequest, CommitResponse,
    DeleteDocumentRequest, DeleteDocumentResponse, GetDocumentRequest, GetDocumentResponse,
    HealthCheckRequest, HealthCheckResponse, PrepareRequest, PrepareResponse,
    QueryDocumentsRequest, QueryDocumentsResponse, ShardInfoRequest, ShardInfoResponse,
    StoreDocumentRequest, StoreDocumentResponse, TransactionStatusRequest,
    TransactionStatusResponse, VectorSearchRequest, VectorSearchResponse,
};
use async_trait::async_trait;
use fen_core::domain::{NodeId, ShardId};
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::debug;

/// Handler trait for processing shard service requests.
///
/// Implement this trait to provide the actual storage and transaction logic.
#[async_trait]
pub trait ShardServiceHandler: Send + Sync + 'static {
    /// Store a document.
    async fn store_document(
        &self,
        request: StoreDocumentRequest,
    ) -> Result<StoreDocumentResponse, Status>;

    /// Get a document.
    async fn get_document(
        &self,
        request: GetDocumentRequest,
    ) -> Result<GetDocumentResponse, Status>;

    /// Delete a document.
    async fn delete_document(
        &self,
        request: DeleteDocumentRequest,
    ) -> Result<DeleteDocumentResponse, Status>;

    /// Query documents.
    async fn query_documents(
        &self,
        request: QueryDocumentsRequest,
    ) -> Result<QueryDocumentsResponse, Status>;

    /// Perform vector search.
    async fn vector_search(
        &self,
        request: VectorSearchRequest,
    ) -> Result<VectorSearchResponse, Status>;

    /// Prepare transaction (2PC phase 1).
    async fn prepare(&self, request: PrepareRequest) -> Result<PrepareResponse, Status>;

    /// Commit transaction (2PC phase 2).
    async fn commit(&self, request: CommitRequest) -> Result<CommitResponse, Status>;

    /// Abort transaction.
    async fn abort(&self, request: AbortRequest) -> Result<AbortResponse, Status>;

    /// Get transaction status.
    async fn get_transaction_status(
        &self,
        request: TransactionStatusRequest,
    ) -> Result<TransactionStatusResponse, Status>;

    /// Health check.
    async fn health_check(
        &self,
        request: HealthCheckRequest,
    ) -> Result<HealthCheckResponse, Status>;

    /// Get shard info.
    async fn get_shard_info(&self, request: ShardInfoRequest) -> Result<ShardInfoResponse, Status>;
}

/// gRPC server for shard service.
pub struct ShardServer<H: ShardServiceHandler> {
    handler: Arc<H>,
    node_id: NodeId,
}

impl<H: ShardServiceHandler> ShardServer<H> {
    /// Create a new shard server.
    pub fn new(handler: H, node_id: NodeId) -> Self {
        Self {
            handler: Arc::new(handler),
            node_id,
        }
    }

    /// Get the node ID.
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }
}

#[tonic::async_trait]
impl<H: ShardServiceHandler> ShardService for ShardServer<H> {
    async fn store_document(
        &self,
        request: Request<StoreDocumentRequest>,
    ) -> Result<Response<StoreDocumentResponse>, Status> {
        debug!("Received store_document request");
        let response = self.handler.store_document(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn get_document(
        &self,
        request: Request<GetDocumentRequest>,
    ) -> Result<Response<GetDocumentResponse>, Status> {
        debug!("Received get_document request");
        let response = self.handler.get_document(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn delete_document(
        &self,
        request: Request<DeleteDocumentRequest>,
    ) -> Result<Response<DeleteDocumentResponse>, Status> {
        debug!("Received delete_document request");
        let response = self.handler.delete_document(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn query_documents(
        &self,
        request: Request<QueryDocumentsRequest>,
    ) -> Result<Response<QueryDocumentsResponse>, Status> {
        debug!("Received query_documents request");
        let response = self.handler.query_documents(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn vector_search(
        &self,
        request: Request<VectorSearchRequest>,
    ) -> Result<Response<VectorSearchResponse>, Status> {
        debug!("Received vector_search request");
        let response = self.handler.vector_search(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn prepare(
        &self,
        request: Request<PrepareRequest>,
    ) -> Result<Response<PrepareResponse>, Status> {
        debug!("Received prepare request");
        let response = self.handler.prepare(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn commit(
        &self,
        request: Request<CommitRequest>,
    ) -> Result<Response<CommitResponse>, Status> {
        debug!("Received commit request");
        let response = self.handler.commit(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn abort(
        &self,
        request: Request<AbortRequest>,
    ) -> Result<Response<AbortResponse>, Status> {
        debug!("Received abort request");
        let response = self.handler.abort(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn get_transaction_status(
        &self,
        request: Request<TransactionStatusRequest>,
    ) -> Result<Response<TransactionStatusResponse>, Status> {
        debug!("Received get_transaction_status request");
        let response = self
            .handler
            .get_transaction_status(request.into_inner())
            .await?;
        Ok(Response::new(response))
    }

    async fn health_check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        debug!("Received health_check request");
        let response = self.handler.health_check(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn get_shard_info(
        &self,
        request: Request<ShardInfoRequest>,
    ) -> Result<Response<ShardInfoResponse>, Status> {
        debug!("Received get_shard_info request");
        let response = self.handler.get_shard_info(request.into_inner()).await?;
        Ok(Response::new(response))
    }
}

/// A simple stub handler for testing.
pub struct StubHandler {
    node_id: String,
    shards: Vec<ShardId>,
}

impl StubHandler {
    /// Create a new stub handler.
    pub fn new(node_id: NodeId, shards: Vec<ShardId>) -> Self {
        Self {
            node_id: node_id.0.to_string(),
            shards,
        }
    }
}

#[async_trait]
impl ShardServiceHandler for StubHandler {
    async fn store_document(
        &self,
        _request: StoreDocumentRequest,
    ) -> Result<StoreDocumentResponse, Status> {
        Ok(StoreDocumentResponse {
            success: true,
            error: String::new(),
            stored_at: None,
        })
    }

    async fn get_document(
        &self,
        _request: GetDocumentRequest,
    ) -> Result<GetDocumentResponse, Status> {
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
        _request: DeleteDocumentRequest,
    ) -> Result<DeleteDocumentResponse, Status> {
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

    async fn prepare(&self, _request: PrepareRequest) -> Result<PrepareResponse, Status> {
        Ok(PrepareResponse {
            vote_commit: true,
            reason: String::new(),
        })
    }

    async fn commit(&self, _request: CommitRequest) -> Result<CommitResponse, Status> {
        Ok(CommitResponse {
            success: true,
            error: String::new(),
        })
    }

    async fn abort(&self, _request: AbortRequest) -> Result<AbortResponse, Status> {
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
            details.insert("node_id".to_string(), self.node_id.clone());
            details.insert("shard_count".to_string(), self.shards.len().to_string());
        }

        Ok(HealthCheckResponse {
            healthy: true,
            status: "serving".to_string(),
            details,
        })
    }

    async fn get_shard_info(&self, request: ShardInfoRequest) -> Result<ShardInfoResponse, Status> {
        let shard_id = request.shard_id.unwrap_or(crate::proto::ShardId { id: 0 });

        Ok(ShardInfoResponse {
            shard_id: Some(shard_id),
            is_primary: true,
            node_id: self.node_id.clone(),
            document_count: 0,
            size_bytes: 0,
            last_updated: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_handler_creation() {
        let handler = StubHandler::new(NodeId::new(), vec![ShardId(0), ShardId(1)]);
        assert_eq!(handler.shards.len(), 2);
    }
}
