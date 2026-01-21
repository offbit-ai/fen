use serde::{Deserialize, Serialize};

use super::ids::DocumentId;

/// Types of anomalies that can be detected
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Math inconsistency (line items don't sum to total)
    MathMismatch,
    /// Missing required field
    MissingField,
    /// Format validation failure
    InvalidFormat,
    /// Value out of expected range
    OutOfRange,
    /// Potential duplicate document
    PotentialDuplicate,
    /// Date inconsistency
    DateInconsistency,
    /// Contract violation
    ContractViolation,
    /// Generic validation failure
    ValidationFailure,
}

impl std::fmt::Display for AnomalyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalyType::MathMismatch => write!(f, "Math Mismatch"),
            AnomalyType::MissingField => write!(f, "Missing Field"),
            AnomalyType::InvalidFormat => write!(f, "Invalid Format"),
            AnomalyType::OutOfRange => write!(f, "Out of Range"),
            AnomalyType::PotentialDuplicate => write!(f, "Potential Duplicate"),
            AnomalyType::DateInconsistency => write!(f, "Date Inconsistency"),
            AnomalyType::ContractViolation => write!(f, "Contract Violation"),
            AnomalyType::ValidationFailure => write!(f, "Validation Failure"),
        }
    }
}

/// Severity levels for anomalies
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Severity {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Low => write!(f, "Low"),
            Severity::Medium => write!(f, "Medium"),
            Severity::High => write!(f, "High"),
            Severity::Critical => write!(f, "Critical"),
        }
    }
}

/// A detected anomaly
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anomaly {
    pub document_id: DocumentId,
    pub anomaly_type: AnomalyType,
    pub severity: Severity,
    pub description: String,
    pub field_path: Option<String>,
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub confidence: f32,
}

impl Anomaly {
    /// Create a new anomaly
    pub fn new(
        document_id: DocumentId,
        anomaly_type: AnomalyType,
        severity: Severity,
        description: impl Into<String>,
    ) -> Self {
        Self {
            document_id,
            anomaly_type,
            severity,
            description: description.into(),
            field_path: None,
            expected_value: None,
            actual_value: None,
            confidence: 1.0,
        }
    }

    /// Set the field path
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field_path = Some(field.into());
        self
    }

    /// Set expected and actual values
    pub fn with_values(
        mut self,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        self.expected_value = Some(expected.into());
        self.actual_value = Some(actual.into());
        self
    }
}

/// Result of validating a document
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    pub document_id: DocumentId,
    pub is_valid: bool,
    pub anomalies: Vec<Anomaly>,
    pub validation_time_ms: u64,
}

impl ValidationResult {
    /// Create a new validation result
    pub fn new(document_id: DocumentId) -> Self {
        Self {
            document_id,
            is_valid: true,
            anomalies: Vec::new(),
            validation_time_ms: 0,
        }
    }

    /// Add an anomaly to the result
    pub fn add_anomaly(&mut self, anomaly: Anomaly) {
        self.anomalies.push(anomaly);
        self.is_valid = false;
    }

    /// Check if there are any critical anomalies
    pub fn has_critical(&self) -> bool {
        self.anomalies.iter().any(|a| a.severity == Severity::Critical)
    }

    /// Check if there are any high severity anomalies
    pub fn has_high_severity(&self) -> bool {
        self.anomalies
            .iter()
            .any(|a| a.severity >= Severity::High)
    }
}
