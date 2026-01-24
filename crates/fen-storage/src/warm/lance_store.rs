use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, Float64Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use futures::stream::TryStreamExt;
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::table::Table;
use lancedb::Connection;

use fen_core::domain::{Invoice, InvoiceId};

use crate::config::WarmStorageBackend;
use crate::error::StorageError;

/// Embedding dimensions for document vectors
pub const EMBEDDING_DIM: i32 = 768;

/// LanceDB-based warm tier storage with vector search capabilities
pub struct LanceStorage {
    conn: Connection,
    invoices_table: Option<Table>,
}

impl LanceStorage {
    /// Create a new LanceDB storage at the given path (embedded mode)
    ///
    /// This is the default constructor for development use. For production,
    /// use `from_backend` with a configured `WarmStorageBackend`.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let backend = WarmStorageBackend::embedded(path.as_ref().to_string_lossy().to_string());
        Self::from_backend(&backend).await
    }

    /// Create LanceDB storage from backend configuration
    ///
    /// This is the preferred constructor for production deployments,
    /// allowing configuration of embedded, S3, or LanceDB Cloud backends.
    ///
    /// # Feature Flags
    ///
    /// - Without `remote-storage`: Only embedded backend is supported
    /// - With `remote-storage`: S3 and LanceDB Cloud backends are available
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Development: embedded storage
    /// let backend = WarmStorageBackend::embedded("/data/warm");
    /// let storage = LanceStorage::from_backend(&backend).await?;
    ///
    /// // Production: S3 storage (requires remote-storage feature)
    /// let backend = WarmStorageBackend::s3("s3://bucket/warm", s3_config);
    /// let storage = LanceStorage::from_backend(&backend).await?;
    /// ```
    pub async fn from_backend(backend: &WarmStorageBackend) -> Result<Self, StorageError> {
        let conn = Self::connect_backend(backend).await?;

        let backend_type = if backend.is_embedded() {
            "embedded"
        } else {
            "remote"
        };

        tracing::info!(
            uri = %backend.connection_uri(),
            backend = %backend_type,
            "Connected to LanceDB"
        );

        let mut storage = Self {
            conn,
            invoices_table: None,
        };

        // Initialize tables
        storage.ensure_tables().await?;

        Ok(storage)
    }

    /// Connect to LanceDB based on backend configuration
    async fn connect_backend(backend: &WarmStorageBackend) -> Result<Connection, StorageError> {
        match backend {
            WarmStorageBackend::Embedded { path } => {
                // Ensure directory exists for embedded mode
                if let Some(parent) = Path::new(path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::create_dir_all(path)?;

                connect(path)
                    .execute()
                    .await
                    .map_err(|e| StorageError::Connection(e.to_string()))
            }

            #[cfg(feature = "remote-storage")]
            WarmStorageBackend::S3 { uri, config } => {
                let mut builder = connect(uri);

                // Configure S3 storage options
                builder = builder.storage_option("region", &config.region);

                if let Some(ref key) = config.access_key_id {
                    builder = builder.storage_option("aws_access_key_id", key);
                }
                if let Some(ref secret) = config.secret_access_key {
                    builder = builder.storage_option("aws_secret_access_key", secret);
                }
                if let Some(ref endpoint) = config.endpoint {
                    builder = builder.storage_option("endpoint", endpoint);
                }
                if config.allow_http {
                    builder = builder.storage_option("allow_http", "true");
                }

                builder
                    .execute()
                    .await
                    .map_err(|e| StorageError::Connection(e.to_string()))
            }

            #[cfg(feature = "remote-storage")]
            WarmStorageBackend::LanceCloud {
                db_uri,
                api_key,
                region,
            } => {
                let mut builder = connect(db_uri).api_key(api_key);

                if let Some(ref r) = region {
                    builder = builder.host_override(r);
                }

                builder
                    .execute()
                    .await
                    .map_err(|e| StorageError::Connection(e.to_string()))
            }
        }
    }

    /// Ensure all required tables exist
    async fn ensure_tables(&mut self) -> Result<(), StorageError> {
        // Check if invoices table exists
        let tables = self
            .conn
            .table_names()
            .execute()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        if tables.contains(&"invoices".to_string()) {
            self.invoices_table = Some(
                self.conn
                    .open_table("invoices")
                    .execute()
                    .await
                    .map_err(|e| StorageError::Database(e.to_string()))?,
            );
        }

        Ok(())
    }

    /// Get or create the invoices table with schema
    async fn get_or_create_invoices_table(&mut self) -> Result<&Table, StorageError> {
        if self.invoices_table.is_none() {
            let schema = Self::invoice_schema();
            // Create empty table with schema
            let batch = Self::empty_invoice_batch(&schema)?;
            let batches = RecordBatchIterator::new(vec![Ok(batch)], Arc::new(schema));

            let table = self
                .conn
                .create_table("invoices", Box::new(batches))
                .execute()
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;

            self.invoices_table = Some(table);
        }

        Ok(self.invoices_table.as_ref().unwrap())
    }

    /// Invoice table schema
    fn invoice_schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("document_id", DataType::Utf8, false),
            Field::new("invoice_number", DataType::Utf8, false),
            Field::new("invoice_date", DataType::Utf8, false),
            Field::new("due_date", DataType::Utf8, true),
            Field::new("vendor_name", DataType::Utf8, true),
            Field::new("bill_to_name", DataType::Utf8, true),
            Field::new("currency", DataType::Utf8, false),
            Field::new("subtotal", DataType::Float64, false),
            Field::new("tax_amount", DataType::Float64, false),
            Field::new("total_amount", DataType::Float64, false),
            Field::new("confidence_score", DataType::Float32, false),
            Field::new("extracted_text", DataType::Utf8, false),
            // JSON-serialized full data for retrieval
            Field::new("data_json", DataType::Utf8, false),
            // Embedding vector for semantic search
            Field::new(
                "embedding",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM,
                ),
                true,
            ),
        ])
    }

    /// Create an empty batch with the invoice schema
    fn empty_invoice_batch(schema: &Schema) -> Result<RecordBatch, StorageError> {
        let embedding_field = Arc::new(Field::new("item", DataType::Float32, true));

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(Vec::<&str>::new())),         // id
            Arc::new(StringArray::from(Vec::<&str>::new())),         // document_id
            Arc::new(StringArray::from(Vec::<&str>::new())),         // invoice_number
            Arc::new(StringArray::from(Vec::<&str>::new())),         // invoice_date
            Arc::new(StringArray::from(Vec::<Option<&str>>::new())), // due_date
            Arc::new(StringArray::from(Vec::<Option<&str>>::new())), // vendor_name
            Arc::new(StringArray::from(Vec::<Option<&str>>::new())), // bill_to_name
            Arc::new(StringArray::from(Vec::<&str>::new())),         // currency
            Arc::new(Float64Array::from(Vec::<f64>::new())),         // subtotal
            Arc::new(Float64Array::from(Vec::<f64>::new())),         // tax_amount
            Arc::new(Float64Array::from(Vec::<f64>::new())),         // total_amount
            Arc::new(Float32Array::from(Vec::<f32>::new())),         // confidence_score
            Arc::new(StringArray::from(Vec::<&str>::new())),         // extracted_text
            Arc::new(StringArray::from(Vec::<&str>::new())),         // data_json
            Arc::new(FixedSizeListArray::new_null(embedding_field, EMBEDDING_DIM, 0)), // embedding
        ];

        RecordBatch::try_new(Arc::new(schema.clone()), columns)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    /// Store an invoice with optional embedding
    pub async fn store_invoice_with_embedding(
        &mut self,
        invoice: &Invoice,
        embedding: Option<&[f32]>,
    ) -> Result<(), StorageError> {
        let table = self.get_or_create_invoices_table().await?;
        let schema = Self::invoice_schema();

        // Convert invoice to record batch
        let batch = Self::invoice_to_batch(invoice, embedding, &schema)?;
        let batches = RecordBatchIterator::new(vec![Ok(batch)], Arc::new(schema));

        table
            .add(Box::new(batches))
            .execute()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        tracing::debug!(invoice_id = %invoice.id, "Stored invoice in LanceDB");
        Ok(())
    }

    /// Convert invoice to Arrow RecordBatch
    fn invoice_to_batch(
        invoice: &Invoice,
        embedding: Option<&[f32]>,
        schema: &Schema,
    ) -> Result<RecordBatch, StorageError> {
        let data_json = serde_json::to_string(invoice)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let subtotal = invoice
            .subtotal
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);
        let tax_amount = invoice
            .tax_amount
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);
        let total_amount = invoice
            .total_amount
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);

        let embedding_field = Arc::new(Field::new("item", DataType::Float32, true));

        let embedding_array: ArrayRef = if let Some(emb) = embedding {
            let values = Float32Array::from(emb.to_vec());
            Arc::new(
                FixedSizeListArray::new(embedding_field, EMBEDDING_DIM, Arc::new(values), None),
            )
        } else {
            Arc::new(FixedSizeListArray::new_null(
                embedding_field,
                EMBEDDING_DIM,
                1,
            ))
        };

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(vec![invoice.id.to_string()])),
            Arc::new(StringArray::from(vec![invoice.document_id.to_string()])),
            Arc::new(StringArray::from(vec![invoice.invoice_number.clone()])),
            Arc::new(StringArray::from(vec![invoice.invoice_date.to_string()])),
            Arc::new(StringArray::from(vec![invoice
                .due_date
                .map(|d| d.to_string())])),
            Arc::new(StringArray::from(vec![Some(invoice.vendor.name.clone())])),
            Arc::new(StringArray::from(vec![Some(invoice.bill_to.name.clone())])),
            Arc::new(StringArray::from(vec![invoice.currency.to_string()])),
            Arc::new(Float64Array::from(vec![subtotal])),
            Arc::new(Float64Array::from(vec![tax_amount])),
            Arc::new(Float64Array::from(vec![total_amount])),
            Arc::new(Float32Array::from(vec![invoice.confidence_score])),
            Arc::new(StringArray::from(vec![invoice.extracted_text.clone()])),
            Arc::new(StringArray::from(vec![data_json])),
            embedding_array,
        ];

        RecordBatch::try_new(Arc::new(schema.clone()), columns)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    /// Search invoices by semantic similarity
    pub async fn search_invoices_by_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Invoice, f32)>, StorageError> {
        let table = self
            .invoices_table
            .as_ref()
            .ok_or_else(|| StorageError::NotFound("Invoices table not initialized".to_string()))?;

        let query = table
            .vector_search(query_embedding.to_vec())
            .map_err(|e| StorageError::Database(e.to_string()))?
            .limit(limit);

        let stream = query
            .execute()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let batches: Vec<RecordBatch> = stream
            .try_collect()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        self.extract_invoices_with_distance(&batches)
    }

    /// Extract invoices from record batches with distance scores
    fn extract_invoices_with_distance(
        &self,
        batches: &[RecordBatch],
    ) -> Result<Vec<(Invoice, f32)>, StorageError> {
        let mut invoices = Vec::new();

        for batch in batches {
            let data_json_col = batch
                .column_by_name("data_json")
                .ok_or_else(|| StorageError::Deserialization("Missing data_json column".into()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    StorageError::Deserialization("data_json is not a string array".into())
                })?;

            let distance_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            for i in 0..batch.num_rows() {
                let json = data_json_col.value(i);
                let invoice: Invoice = serde_json::from_str(json)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                let distance = distance_col.map(|c| c.value(i)).unwrap_or(0.0);
                invoices.push((invoice, distance));
            }
        }

        Ok(invoices)
    }

    /// Extract invoices from record batches
    fn extract_invoices(&self, batches: &[RecordBatch]) -> Result<Vec<Invoice>, StorageError> {
        let mut invoices = Vec::new();

        for batch in batches {
            let data_json_col = batch
                .column_by_name("data_json")
                .ok_or_else(|| StorageError::Deserialization("Missing data_json column".into()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    StorageError::Deserialization("data_json is not a string array".into())
                })?;

            for i in 0..batch.num_rows() {
                let json = data_json_col.value(i);
                let invoice: Invoice = serde_json::from_str(json)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                invoices.push(invoice);
            }
        }

        Ok(invoices)
    }

    /// Search invoices with filter
    pub async fn search_invoices_filtered(
        &self,
        query_embedding: Option<&[f32]>,
        filter: &str,
        limit: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        let table = self
            .invoices_table
            .as_ref()
            .ok_or_else(|| StorageError::NotFound("Invoices table not initialized".to_string()))?;

        let batches: Vec<RecordBatch> = if let Some(embedding) = query_embedding {
            // Hybrid search: vector + filter
            let query = table
                .vector_search(embedding.to_vec())
                .map_err(|e| StorageError::Database(e.to_string()))?
                .limit(limit)
                .only_if(filter);

            query
                .execute()
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?
                .try_collect()
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?
        } else {
            // SQL filter only
            let query = table.query().only_if(filter).limit(limit);

            query
                .execute()
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?
                .try_collect()
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?
        };

        self.extract_invoices(&batches)
    }

    /// Get an invoice by ID
    pub async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError> {
        let table = match &self.invoices_table {
            Some(t) => t,
            None => return Ok(None),
        };

        let filter = format!("id = '{}'", id);
        let query = table.query().only_if(filter).limit(1);

        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let invoices = self.extract_invoices(&batches)?;
        Ok(invoices.into_iter().next())
    }

    /// Delete an invoice by ID
    ///
    /// Returns `true` if the invoice was found and deleted, `false` if not found.
    /// Note: LanceDB 0.17 delete returns () so we check existence first.
    pub async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
        let table = match &self.invoices_table {
            Some(t) => t,
            None => return Ok(false),
        };

        // Check if invoice exists first (LanceDB delete doesn't return affected rows)
        let exists = self.get_invoice(id).await?.is_some();
        if !exists {
            return Ok(false);
        }

        // Use SQL predicate to delete by ID
        let predicate = format!("id = '{}'", id);

        table
            .delete(&predicate)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        tracing::debug!(invoice_id = %id, "Deleted invoice from LanceDB");

        Ok(true)
    }

    /// List invoices with pagination
    pub async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        let table = match &self.invoices_table {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };

        // Note: LanceDB doesn't have direct offset support, so we fetch limit+offset and skip
        let query = table.query().limit(limit + offset);

        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let all_invoices = self.extract_invoices(&batches)?;
        Ok(all_invoices.into_iter().skip(offset).take(limit).collect())
    }

    /// Count total invoices
    pub async fn count_invoices(&self) -> Result<usize, StorageError> {
        let table = match &self.invoices_table {
            Some(t) => t,
            None => return Ok(0),
        };

        let count = table
            .count_rows(None)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(count)
    }

    /// Create vector index on embedding column for faster search
    pub async fn create_vector_index(&self) -> Result<(), StorageError> {
        let table = self
            .invoices_table
            .as_ref()
            .ok_or_else(|| StorageError::NotFound("Invoices table not initialized".to_string()))?;

        // Create IVF-PQ index for efficient vector search
        table
            .create_index(&["embedding"], lancedb::index::Index::Auto)
            .execute()
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        tracing::info!("Created vector index on invoices table");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_lance_storage_create() {
        let dir = tempdir().unwrap();
        let storage: Result<LanceStorage, StorageError> = LanceStorage::new(dir.path()).await;
        assert!(storage.is_ok());
    }

    #[tokio::test]
    async fn test_lance_storage_store_and_retrieve() {
        let dir = tempdir().unwrap();
        let mut storage: LanceStorage = LanceStorage::new(dir.path()).await.unwrap();

        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());

        // Store without embedding
        let _: () = storage
            .store_invoice_with_embedding(&invoice, None)
            .await
            .unwrap();

        // Retrieve
        let loaded: Option<Invoice> = storage.get_invoice(&invoice.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().invoice_number, "INV-001");
    }

    #[tokio::test]
    async fn test_lance_storage_vector_search() {
        let dir = tempdir().unwrap();
        let mut storage: LanceStorage = LanceStorage::new(dir.path()).await.unwrap();

        let invoice = Invoice::new("INV-002", NaiveDate::from_ymd_opt(2024, 2, 1).unwrap());

        // Create a dummy embedding
        let embedding: Vec<f32> = (0..EMBEDDING_DIM).map(|i| i as f32 / 1000.0).collect();

        let _: () = storage
            .store_invoice_with_embedding(&invoice, Some(&embedding))
            .await
            .unwrap();

        // Search with similar embedding
        let query: Vec<f32> = (0..EMBEDDING_DIM).map(|i| i as f32 / 1000.0 + 0.001).collect();
        let results: Vec<(Invoice, f32)> = storage.search_invoices_by_embedding(&query, 10).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.invoice_number, "INV-002");
    }

    #[tokio::test]
    async fn test_lance_storage_delete() {
        let dir = tempdir().unwrap();
        let mut storage = LanceStorage::new(dir.path()).await.unwrap();

        let invoice = Invoice::new("INV-DELETE", NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());

        // Store the invoice
        storage
            .store_invoice_with_embedding(&invoice, None)
            .await
            .unwrap();

        // Verify it exists
        assert!(storage.get_invoice(&invoice.id).await.unwrap().is_some());

        // Delete it
        let deleted = storage.delete_invoice(&invoice.id).await.unwrap();
        assert!(deleted);

        // Verify it's gone
        assert!(storage.get_invoice(&invoice.id).await.unwrap().is_none());

        // Deleting again should return false
        let deleted_again = storage.delete_invoice(&invoice.id).await.unwrap();
        assert!(!deleted_again);
    }
}
