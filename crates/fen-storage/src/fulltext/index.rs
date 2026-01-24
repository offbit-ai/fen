//! Tantivy-based full-text index implementation

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, TEXT, STRING};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};
use tokio::sync::RwLock;

use fen_core::domain::{Contract, Invoice, InvoiceId, ContractId};
use crate::error::StorageError;

/// Full-text search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Document ID
    pub id: String,
    /// BM25 relevance score
    pub score: f32,
}

/// Configuration for the full-text index
#[derive(Debug, Clone)]
pub struct FullTextConfig {
    /// Memory budget for the index writer (bytes)
    pub writer_heap_size: usize,
    /// Number of indexing threads
    pub num_threads: usize,
    /// BM25 k1 parameter (term frequency saturation)
    pub bm25_k1: f32,
    /// BM25 b parameter (document length normalization)
    pub bm25_b: f32,
}

impl Default for FullTextConfig {
    fn default() -> Self {
        Self {
            writer_heap_size: 50_000_000, // 50MB
            num_threads: 1,
            bm25_k1: 1.2,
            bm25_b: 0.75,
        }
    }
}

/// BM25 scorer with corpus statistics for accurate relevance scoring
///
/// Implements the Okapi BM25 ranking function with proper IDF calculation
/// using the Robertson-Sparck Jones formula.
pub struct BM25Scorer {
    /// Total number of documents in the corpus
    doc_count: AtomicU64,
    /// Sum of all document lengths
    total_doc_length: AtomicU64,
    /// Document frequency for each term (number of docs containing the term)
    doc_frequencies: DashMap<String, u64>,
    /// BM25 k1 parameter (controls term frequency saturation, typical: 1.2-2.0)
    k1: f32,
    /// BM25 b parameter (controls document length normalization, typical: 0.75)
    b: f32,
}

impl BM25Scorer {
    /// Create a new BM25 scorer with default parameters
    pub fn new() -> Self {
        Self::with_params(1.2, 0.75)
    }

    /// Create a new BM25 scorer with custom parameters
    pub fn with_params(k1: f32, b: f32) -> Self {
        Self {
            doc_count: AtomicU64::new(0),
            total_doc_length: AtomicU64::new(0),
            doc_frequencies: DashMap::new(),
            k1,
            b,
        }
    }

    /// Add a document to the corpus statistics
    pub fn add_document(&self, text: &str) {
        let tokens = tokenize(text);
        let doc_len = tokens.len() as u64;

        self.doc_count.fetch_add(1, Ordering::SeqCst);
        self.total_doc_length.fetch_add(doc_len, Ordering::SeqCst);

        // Track unique terms in this document for document frequency
        let unique_terms: std::collections::HashSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
        for term in unique_terms {
            self.doc_frequencies
                .entry(term.to_lowercase())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    /// Remove a document from the corpus statistics
    pub fn remove_document(&self, text: &str) {
        let tokens = tokenize(text);
        let doc_len = tokens.len() as u64;

        self.doc_count.fetch_sub(1, Ordering::SeqCst);
        self.total_doc_length.fetch_sub(doc_len, Ordering::SeqCst);

        // Decrease document frequencies for unique terms
        let unique_terms: std::collections::HashSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
        for term in unique_terms {
            if let Some(mut entry) = self.doc_frequencies.get_mut(&term.to_lowercase()) {
                if *entry > 0 {
                    *entry -= 1;
                }
            }
        }
    }

    /// Get the average document length
    pub fn avg_doc_length(&self) -> f32 {
        let total = self.total_doc_length.load(Ordering::SeqCst) as f32;
        let count = self.doc_count.load(Ordering::SeqCst) as f32;
        if count > 0.0 { total / count } else { 1.0 }
    }

    /// Get the total document count
    pub fn doc_count(&self) -> u64 {
        self.doc_count.load(Ordering::SeqCst)
    }

    /// Calculate IDF using Robertson-Sparck Jones formula
    ///
    /// IDF = ln((N - df + 0.5) / (df + 0.5) + 1)
    ///
    /// Where N is total docs and df is document frequency
    fn idf(&self, term: &str) -> f32 {
        let n = self.doc_count() as f32;
        let df = self.doc_frequencies
            .get(&term.to_lowercase())
            .map(|v| *v as f32)
            .unwrap_or(0.0);

        if n == 0.0 || df == 0.0 {
            return 0.0;
        }

        // Robertson-Sparck Jones formula with +1 to ensure non-negative
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Calculate BM25 score for a document against a query
    ///
    /// BM25 = sum(IDF(qi) * (f(qi, D) * (k1 + 1)) / (f(qi, D) + k1 * (1 - b + b * |D| / avgdl)))
    ///
    /// Where:
    /// - qi is a query term
    /// - f(qi, D) is the term frequency in document D
    /// - |D| is the document length
    /// - avgdl is the average document length
    pub fn score(&self, text: &str, query: &str) -> f32 {
        let tokens = tokenize(text);
        let doc_len = tokens.len() as f32;
        let avg_dl = self.avg_doc_length();

        // Build term frequency map for the document
        let mut term_freq: HashMap<String, u32> = HashMap::new();
        for token in &tokens {
            *term_freq.entry(token.to_lowercase()).or_insert(0) += 1;
        }

        // Score each query term
        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return 0.0;
        }

        let mut total_score = 0.0f32;

        for qterm in &query_terms {
            let qterm_lower = qterm.to_lowercase();
            let tf = term_freq.get(&qterm_lower).copied().unwrap_or(0) as f32;

            if tf > 0.0 {
                let idf = self.idf(&qterm_lower);
                // BM25 term score formula
                let numerator = tf * (self.k1 + 1.0);
                let denominator = tf + self.k1 * (1.0 - self.b + self.b * doc_len / avg_dl);
                total_score += idf * (numerator / denominator);
            }
        }

        total_score
    }

    /// Check if text contains all query terms
    pub fn contains(&self, text: &str, query: &str) -> bool {
        let text_tokens: std::collections::HashSet<String> = tokenize(text)
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect();
        let query_tokens = tokenize(query);

        query_tokens.iter().all(|qt| text_tokens.contains(&qt.to_lowercase()))
    }
}

impl Default for BM25Scorer {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple tokenizer - splits on whitespace and punctuation, lowercases
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty() && s.len() >= 2)  // Filter out single chars and empty
        .map(|s| s.to_lowercase())
        .collect()
}

/// Full-text search index using Tantivy
pub struct FullTextIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
    #[allow(dead_code)]
    schema: Schema,
    // Schema fields
    id_field: Field,
    doc_type_field: Field,
    text_field: Field,
    invoice_number_field: Field,
    vendor_name_field: Field,
    // BM25 scorer for score fusion
    bm25_scorer: Arc<BM25Scorer>,
}

impl FullTextIndex {
    /// Create a new in-memory full-text index
    pub fn in_memory(config: FullTextConfig) -> Result<Self, StorageError> {
        let schema = Self::build_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, schema, config)
    }

    /// Create a new file-based full-text index
    pub fn new(path: &Path, config: FullTextConfig) -> Result<Self, StorageError> {
        std::fs::create_dir_all(path).map_err(|e| {
            StorageError::Index(format!("Failed to create index directory: {}", e))
        })?;

        let schema = Self::build_schema();
        let index = Index::create_in_dir(path, schema.clone()).map_err(|e| {
            StorageError::Index(format!("Failed to create Tantivy index: {}", e))
        })?;

        Self::from_index(index, schema, config)
    }

    /// Open an existing file-based full-text index
    pub fn open(path: &Path, config: FullTextConfig) -> Result<Self, StorageError> {
        let index = Index::open_in_dir(path).map_err(|e| {
            StorageError::Index(format!("Failed to open Tantivy index: {}", e))
        })?;

        let schema = index.schema();
        Self::from_index(index, schema, config)
    }

    fn from_index(index: Index, schema: Schema, config: FullTextConfig) -> Result<Self, StorageError> {
        let writer = index
            .writer_with_num_threads(config.num_threads, config.writer_heap_size)
            .map_err(|e| StorageError::Index(format!("Failed to create index writer: {}", e)))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| StorageError::Index(format!("Failed to create index reader: {}", e)))?;

        let id_field = schema.get_field("id").unwrap();
        let doc_type_field = schema.get_field("doc_type").unwrap();
        let text_field = schema.get_field("text").unwrap();
        let invoice_number_field = schema.get_field("invoice_number").unwrap();
        let vendor_name_field = schema.get_field("vendor_name").unwrap();

        let bm25_scorer = Arc::new(BM25Scorer::with_params(config.bm25_k1, config.bm25_b));

        Ok(Self {
            index,
            reader,
            writer: Arc::new(RwLock::new(writer)),
            schema,
            id_field,
            doc_type_field,
            text_field,
            invoice_number_field,
            vendor_name_field,
            bm25_scorer,
        })
    }

    fn build_schema() -> Schema {
        let mut schema_builder = Schema::builder();

        // Document ID (stored for retrieval)
        schema_builder.add_text_field("id", STRING | STORED);

        // Document type (invoice/contract)
        schema_builder.add_text_field("doc_type", STRING | STORED);

        // Main text content (searchable with BM25)
        schema_builder.add_text_field("text", TEXT);

        // Invoice number (searchable)
        schema_builder.add_text_field("invoice_number", TEXT | STORED);

        // Vendor name (searchable)
        schema_builder.add_text_field("vendor_name", TEXT | STORED);

        schema_builder.build()
    }

    /// Index an invoice
    pub async fn index_invoice(&self, invoice: &Invoice) -> Result<(), StorageError> {
        let writer = self.writer.write().await;

        // Build searchable text from invoice fields
        let text = build_invoice_text(invoice);

        // Update BM25 corpus statistics
        self.bm25_scorer.add_document(&text);

        let doc = doc!(
            self.id_field => invoice.id.0.to_string(),
            self.doc_type_field => "invoice",
            self.text_field => text,
            self.invoice_number_field => invoice.invoice_number.clone(),
            self.vendor_name_field => invoice.vendor.name.clone()
        );

        writer.add_document(doc).map_err(|e| {
            StorageError::Index(format!("Failed to index invoice: {}", e))
        })?;

        Ok(())
    }

    /// Index a contract
    pub async fn index_contract(&self, contract: &Contract) -> Result<(), StorageError> {
        let writer = self.writer.write().await;

        // Build searchable text from contract fields
        let text = build_contract_text(contract);

        // Update BM25 corpus statistics
        self.bm25_scorer.add_document(&text);

        let doc = doc!(
            self.id_field => contract.id.0.to_string(),
            self.doc_type_field => "contract",
            self.text_field => text,
            self.invoice_number_field => contract.title.clone(),
            self.vendor_name_field => contract.parties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
        );

        writer.add_document(doc).map_err(|e| {
            StorageError::Index(format!("Failed to index contract: {}", e))
        })?;

        Ok(())
    }

    /// Delete an invoice from the index
    pub async fn delete_invoice(&self, id: &InvoiceId) -> Result<(), StorageError> {
        let writer = self.writer.write().await;
        let term = tantivy::Term::from_field_text(self.id_field, &id.0.to_string());
        writer.delete_term(term);
        Ok(())
    }

    /// Delete a contract from the index
    pub async fn delete_contract(&self, id: &ContractId) -> Result<(), StorageError> {
        let writer = self.writer.write().await;
        let term = tantivy::Term::from_field_text(self.id_field, &id.0.to_string());
        writer.delete_term(term);
        Ok(())
    }

    /// Commit pending changes and reload reader
    pub async fn commit(&self) -> Result<(), StorageError> {
        let mut writer = self.writer.write().await;
        writer.commit().map_err(|e| {
            StorageError::Index(format!("Failed to commit index: {}", e))
        })?;

        // Reload the reader to see the committed changes
        self.reader.reload().map_err(|e| {
            StorageError::Index(format!("Failed to reload index reader: {}", e))
        })?;

        Ok(())
    }

    /// Search invoices by text query
    pub fn search_invoices(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, StorageError> {
        self.search_with_filter(query_str, "invoice", limit)
    }

    /// Search contracts by text query
    pub fn search_contracts(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, StorageError> {
        self.search_with_filter(query_str, "contract", limit)
    }

    /// Search all documents by text query
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, StorageError> {
        let searcher = self.reader.searcher();

        let query_parser = QueryParser::for_index(&self.index, vec![self.text_field, self.invoice_number_field, self.vendor_name_field]);

        let query = query_parser.parse_query(query_str).map_err(|e| {
            StorageError::Query(format!("Failed to parse query: {}", e))
        })?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit)).map_err(|e| {
            StorageError::Query(format!("Search failed: {}", e))
        })?;

        let results = top_docs
            .into_iter()
            .filter_map(|(score, doc_address)| {
                let doc: TantivyDocument = searcher.doc(doc_address).ok()?;
                let id = doc.get_first(self.id_field)?.as_str()?.to_string();
                Some(SearchResult { id, score })
            })
            .collect();

        Ok(results)
    }

    fn search_with_filter(&self, query_str: &str, doc_type: &str, limit: usize) -> Result<Vec<SearchResult>, StorageError> {
        let searcher = self.reader.searcher();

        // Combine user query with doc_type filter
        let full_query = format!("({}) AND doc_type:{}", query_str, doc_type);

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.text_field, self.invoice_number_field, self.vendor_name_field, self.doc_type_field]
        );

        let query = query_parser.parse_query(&full_query).map_err(|e| {
            StorageError::Query(format!("Failed to parse query: {}", e))
        })?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit)).map_err(|e| {
            StorageError::Query(format!("Search failed: {}", e))
        })?;

        let results = top_docs
            .into_iter()
            .filter_map(|(score, doc_address)| {
                let doc: TantivyDocument = searcher.doc(doc_address).ok()?;
                let id = doc.get_first(self.id_field)?.as_str()?.to_string();
                Some(SearchResult { id, score })
            })
            .collect();

        Ok(results)
    }

    /// Check if a text contains all the given search terms
    ///
    /// Uses proper tokenization to match terms accurately
    pub fn contains(&self, text: &str, search_terms: &str) -> bool {
        self.bm25_scorer.contains(text, search_terms)
    }

    /// Calculate BM25 score for a document against a query
    ///
    /// Uses the Okapi BM25 ranking function with proper IDF calculation
    /// based on corpus statistics collected from indexed documents.
    ///
    /// The score reflects how relevant the text is to the query terms,
    /// considering:
    /// - Term frequency in the document
    /// - Inverse document frequency (rarity of terms across corpus)
    /// - Document length normalization
    pub fn bm25_score(&self, text: &str, query: &str) -> f32 {
        self.bm25_scorer.score(text, query)
    }

    /// Get the BM25 scorer for direct access to corpus statistics
    pub fn scorer(&self) -> &BM25Scorer {
        &self.bm25_scorer
    }

    /// Get corpus statistics
    pub fn corpus_stats(&self) -> (u64, f32) {
        (self.bm25_scorer.doc_count(), self.bm25_scorer.avg_doc_length())
    }
}

/// Build searchable text from an invoice
fn build_invoice_text(invoice: &Invoice) -> String {
    let mut parts = Vec::new();

    parts.push(invoice.invoice_number.clone());
    parts.push(invoice.vendor.name.clone());
    parts.push(invoice.bill_to.name.clone());

    if let Some(po) = &invoice.po_number {
        parts.push(po.clone());
    }

    if let Some(tax_id) = &invoice.vendor.tax_id {
        parts.push(tax_id.clone());
    }

    // Add line item descriptions
    for item in &invoice.line_items {
        parts.push(item.description.clone());
    }

    // Add extracted text
    if !invoice.extracted_text.is_empty() {
        parts.push(invoice.extracted_text.clone());
    }

    parts.join(" ")
}

/// Build searchable text from a contract
fn build_contract_text(contract: &Contract) -> String {
    let mut parts = Vec::new();

    parts.push(contract.title.clone());

    if let Some(num) = &contract.contract_number {
        parts.push(num.clone());
    }

    // Add party names
    for party in &contract.parties {
        parts.push(party.name.clone());
    }

    // Add clause text
    for clause in &contract.clauses {
        parts.push(clause.text.clone());
        if let Some(title) = &clause.title {
            parts.push(title.clone());
        }
    }

    // Add extracted text
    if !contract.extracted_text.is_empty() {
        parts.push(contract.extracted_text.clone());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use fen_core::domain::Party;

    fn test_invoice() -> Invoice {
        let mut invoice = Invoice::new("INV-001", Utc::now().date_naive());
        invoice.vendor = Party::new("Acme Corp");
        invoice.extracted_text = "Payment due within 30 days".to_string();
        invoice
    }

    #[tokio::test]
    async fn test_index_and_search() {
        let index = FullTextIndex::in_memory(FullTextConfig::default()).unwrap();

        let invoice = test_invoice();
        index.index_invoice(&invoice).await.unwrap();
        index.commit().await.unwrap();

        // Search by vendor name
        let results = index.search_invoices("Acme", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, invoice.id.0.to_string());

        // Search by invoice number
        let results = index.search_invoices("INV-001", 10).unwrap();
        assert_eq!(results.len(), 1);

        // Search by extracted text
        let results = index.search_invoices("payment", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_contains() {
        let index = FullTextIndex::in_memory(FullTextConfig::default()).unwrap();

        assert!(index.contains("Hello world test document", "hello"));
        assert!(index.contains("Hello world test document", "world test"));
        assert!(!index.contains("Hello world test document", "foo"));
        // Single character terms are filtered out
        assert!(index.contains("Hello world", "hello world"));
    }

    #[test]
    fn test_bm25_score_basic() {
        let index = FullTextIndex::in_memory(FullTextConfig::default()).unwrap();

        // Add some documents to build corpus statistics
        let docs = vec![
            "The quick brown fox jumps over the lazy dog",
            "A quick brown dog runs in the park",
            "The lazy cat sleeps all day long",
            "Dogs and foxes are both animals",
        ];

        for doc in &docs {
            index.bm25_scorer.add_document(doc);
        }

        // Check corpus stats
        assert_eq!(index.bm25_scorer.doc_count(), 4);

        // Score a document
        let text = "The quick brown fox jumps over the lazy dog";
        let score1 = index.bm25_score(text, "quick fox");
        let score2 = index.bm25_score(text, "elephant");
        let score3 = index.bm25_score(text, "lazy dog");

        assert!(score1 > 0.0, "Score for matching terms should be positive");
        assert!(score2 == 0.0, "Score for non-matching term should be zero");
        assert!(score3 > 0.0, "Score for common terms should be positive");

        // "quick fox" should score higher than "lazy dog" because "quick" and "fox"
        // are less common in the corpus
        assert!(score1 > score3, "Rare terms should score higher than common terms");
    }

    #[test]
    fn test_bm25_idf() {
        let scorer = BM25Scorer::new();

        // Add documents where "rare" appears once and "common" appears multiple times
        scorer.add_document("This document has a rare term");
        scorer.add_document("This document has common words");
        scorer.add_document("Another document with common words");
        scorer.add_document("Yet another common document");

        // Score same document against rare vs common term
        let doc = "This has both rare and common terms";
        let rare_score = scorer.score(doc, "rare");
        let common_score = scorer.score(doc, "common");

        // Rare term should have higher IDF and thus higher score
        assert!(rare_score > common_score,
            "Rare terms (IDF higher) should score higher: rare={} vs common={}",
            rare_score, common_score);
    }

    #[test]
    fn test_bm25_term_frequency() {
        let scorer = BM25Scorer::new();

        // Add multiple documents to build corpus statistics
        scorer.add_document("sample document for testing");
        scorer.add_document("another document with different words");
        scorer.add_document("test document here");
        scorer.add_document("test test test test document repeated");

        // Document with repeated term
        let doc_repeated = "test test test test document";
        let doc_single = "test document here";

        let score_repeated = scorer.score(doc_repeated, "test");
        let score_single = scorer.score(doc_single, "test");

        // More occurrences should lead to higher score (but with saturation)
        assert!(score_repeated > score_single,
            "Higher term frequency should increase score: repeated={} vs single={}",
            score_repeated, score_single);
    }

    #[tokio::test]
    async fn test_corpus_stats_tracking() {
        let index = FullTextIndex::in_memory(FullTextConfig::default()).unwrap();

        // Initially no documents
        let (count, _avg_len) = index.corpus_stats();
        assert_eq!(count, 0);

        // Add an invoice
        let invoice = test_invoice();
        index.index_invoice(&invoice).await.unwrap();

        // Verify corpus stats updated
        let (count, avg_len) = index.corpus_stats();
        assert_eq!(count, 1);
        assert!(avg_len > 0.0, "Average doc length should be positive");
    }
}
