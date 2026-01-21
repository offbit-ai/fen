use serde::{Deserialize, Serialize};

/// Validation status of a document
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ValidationStatus {
    #[default]
    Pending,
    Passed,
    PassedWithWarnings,
    Failed,
}

impl std::fmt::Display for ValidationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationStatus::Pending => write!(f, "Pending"),
            ValidationStatus::Passed => write!(f, "Passed"),
            ValidationStatus::PassedWithWarnings => write!(f, "Passed with Warnings"),
            ValidationStatus::Failed => write!(f, "Failed"),
        }
    }
}
