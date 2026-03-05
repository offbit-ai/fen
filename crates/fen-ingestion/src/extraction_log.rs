//! Extraction event logging for GLiNER fine-tuning data collection.
//!
//! Logs successful entity extractions as JSONL, capturing (text, label, span) triples
//! that can be used to fine-tune GLiNER models on production invoice data.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Which extraction tier produced the entity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionSource {
    LayoutLmv3,
    GlinerMedium,
    GlinerLarge,
    RuleEngine,
    Regex,
}

/// A single entity extracted from text, with character offsets for training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingEntity {
    pub label: String,
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
}

/// A complete extraction event — one JSONL line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionEvent {
    pub document_id: String,
    pub text: String,
    pub entities: Vec<TrainingEntity>,
    pub source: ExtractionSource,
    pub confidence: f32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Configuration for extraction logging.
#[derive(Debug, Clone)]
pub struct ExtractionLogConfig {
    /// Directory to write JSONL log files.
    pub log_dir: PathBuf,
    /// Minimum confidence to log an extraction event.
    pub min_confidence: f32,
    /// Whether logging is enabled.
    pub enabled: bool,
}

impl Default for ExtractionLogConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("extraction_logs"),
            min_confidence: 0.7,
            enabled: false,
        }
    }
}

/// Writes extraction events as JSONL for training data collection.
pub struct ExtractionLogger {
    config: ExtractionLogConfig,
    writer: Mutex<Option<std::io::BufWriter<std::fs::File>>>,
}

impl ExtractionLogger {
    pub fn new(config: ExtractionLogConfig) -> Result<Self, std::io::Error> {
        if !config.enabled {
            return Ok(Self {
                config,
                writer: Mutex::new(None),
            });
        }

        std::fs::create_dir_all(&config.log_dir)?;

        let filename = format!(
            "extractions_{}.jsonl",
            chrono::Utc::now().format("%Y%m%d")
        );
        let path = config.log_dir.join(filename);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        tracing::info!(path = %path.display(), "Extraction logger initialized");

        Ok(Self {
            config,
            writer: Mutex::new(Some(std::io::BufWriter::new(file))),
        })
    }

    /// Log an extraction event if confidence meets threshold.
    pub fn log(&self, event: &ExtractionEvent) {
        if !self.config.enabled || event.confidence < self.config.min_confidence {
            return;
        }
        if event.entities.is_empty() {
            return;
        }

        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to acquire extraction log lock");
                return;
            }
        };

        if let Some(writer) = guard.as_mut() {
            use std::io::Write;
            match serde_json::to_string(event) {
                Ok(line) => {
                    if let Err(e) = writeln!(writer, "{}", line) {
                        tracing::warn!(error = %e, "Failed to write extraction log");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize extraction event");
                }
            }
        }
    }

    /// Build training entities from an invoice by finding field values in the source text.
    pub fn entities_from_invoice(
        text: &str,
        invoice: &fen_core::domain::Invoice,
    ) -> Vec<TrainingEntity> {
        let mut entities = Vec::new();

        // Invoice number
        if !invoice.invoice_number.starts_with("UNKNOWN-") {
            if let Some((start, end)) = find_span(text, &invoice.invoice_number) {
                entities.push(TrainingEntity {
                    label: "invoice_number".to_string(),
                    text: invoice.invoice_number.clone(),
                    char_start: start,
                    char_end: end,
                });
            }
        }

        // Vendor name
        if invoice.vendor.name != "Unknown" {
            if let Some((start, end)) = find_span(text, &invoice.vendor.name) {
                entities.push(TrainingEntity {
                    label: "vendor_name".to_string(),
                    text: invoice.vendor.name.clone(),
                    char_start: start,
                    char_end: end,
                });
            }
        }

        // Bill-to name
        if invoice.bill_to.name != "Unknown" {
            if let Some((start, end)) = find_span(text, &invoice.bill_to.name) {
                entities.push(TrainingEntity {
                    label: "bill_to_name".to_string(),
                    text: invoice.bill_to.name.clone(),
                    char_start: start,
                    char_end: end,
                });
            }
        }

        // Total amount
        if !invoice.total_amount.is_zero() {
            let amount_str = invoice.total_amount.to_string();
            if let Some((start, end)) = find_amount_span(text, &amount_str) {
                entities.push(TrainingEntity {
                    label: "total_amount".to_string(),
                    text: amount_str,
                    char_start: start,
                    char_end: end,
                });
            }
        }

        // PO number
        if let Some(po) = &invoice.po_number {
            if let Some((start, end)) = find_span(text, po) {
                entities.push(TrainingEntity {
                    label: "po_number".to_string(),
                    text: po.clone(),
                    char_start: start,
                    char_end: end,
                });
            }
        }

        entities
    }
}

/// Find the character span of `needle` in `haystack`.
fn find_span(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    haystack.find(needle).map(|start| (start, start + needle.len()))
}

/// Find an amount value in text, handling currency symbols and formatting.
fn find_amount_span(haystack: &str, amount: &str) -> Option<(usize, usize)> {
    // Try exact match first
    if let Some(span) = find_span(haystack, amount) {
        return Some(span);
    }
    // Try with common currency prefixes
    for prefix in &["$", "€", "£", "¥"] {
        let formatted = format!("{}{}", prefix, amount);
        if let Some(span) = find_span(haystack, &formatted) {
            return Some(span);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_event_serialization() {
        let event = ExtractionEvent {
            document_id: "doc-123".to_string(),
            text: "Invoice #: INV-001\nTotal: $100.00".to_string(),
            entities: vec![
                TrainingEntity {
                    label: "invoice_number".to_string(),
                    text: "INV-001".to_string(),
                    char_start: 11,
                    char_end: 18,
                },
            ],
            source: ExtractionSource::LayoutLmv3,
            confidence: 0.85,
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("INV-001"));
        assert!(json.contains("layout_lmv3"));

        let deserialized: ExtractionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entities.len(), 1);
        assert_eq!(deserialized.entities[0].label, "invoice_number");
    }

    #[test]
    fn test_find_span() {
        assert_eq!(find_span("Invoice #: INV-001", "INV-001"), Some((11, 18)));
        assert_eq!(find_span("no match here", "INV-001"), None);
    }

    #[test]
    fn test_find_amount_span() {
        assert_eq!(find_amount_span("Total: $100.00", "100.00"), Some((8, 14)));
        assert_eq!(find_amount_span("Total: 100.00", "100.00"), Some((7, 13)));
    }

    #[test]
    fn test_disabled_logger() {
        let config = ExtractionLogConfig::default();
        assert!(!config.enabled);

        let logger = ExtractionLogger::new(config).unwrap();
        // Should not panic when logging while disabled
        let event = ExtractionEvent {
            document_id: "test".to_string(),
            text: "test".to_string(),
            entities: vec![],
            source: ExtractionSource::Regex,
            confidence: 0.9,
            timestamp: chrono::Utc::now(),
        };
        logger.log(&event);
    }

    #[test]
    fn test_below_threshold_not_logged() {
        let config = ExtractionLogConfig {
            min_confidence: 0.7,
            enabled: true,
            log_dir: std::env::temp_dir().join("fen_test_extraction_logs"),
        };

        let logger = ExtractionLogger::new(config).unwrap();

        // Low confidence — should not write
        let event = ExtractionEvent {
            document_id: "test".to_string(),
            text: "test".to_string(),
            entities: vec![TrainingEntity {
                label: "test".to_string(),
                text: "val".to_string(),
                char_start: 0,
                char_end: 3,
            }],
            source: ExtractionSource::Regex,
            confidence: 0.3,
            timestamp: chrono::Utc::now(),
        };
        logger.log(&event); // Should silently skip
    }
}
