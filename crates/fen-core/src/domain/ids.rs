use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type-safe document ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub Uuid);

impl DocumentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type-safe invoice ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InvoiceId(pub Uuid);

impl InvoiceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for InvoiceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InvoiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type-safe contract ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractId(pub Uuid);

impl ContractId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for ContractId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ContractId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type-safe vendor/party ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartyId(pub Uuid);

impl PartyId {
    /// Create a new random party ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a nil (zero) party ID for unknown/placeholder parties
    /// This avoids UUID generation overhead for placeholder values
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Check if this is a nil/unknown party ID
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for PartyId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PartyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
