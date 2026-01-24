use serde::{Deserialize, Serialize};

use super::ids::PartyId;

/// Constant name for unknown parties - avoids repeated allocation
const UNKNOWN_PARTY_NAME: &str = "Unknown";

/// Address information
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Address {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

/// Contact information
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Contact {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// Party (vendor, customer, etc.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Party {
    pub id: PartyId,
    pub name: String,
    pub tax_id: Option<String>,
    pub address: Option<Address>,
    pub contact: Option<Contact>,
}

impl Party {
    /// Create a new party with just a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: PartyId::new(),
            name: name.into(),
            tax_id: None,
            address: None,
            contact: None,
        }
    }

    /// Create an unknown/placeholder party
    /// Uses a nil UUID to avoid UUID generation overhead
    pub fn unknown() -> Self {
        Self {
            id: PartyId::nil(),
            name: UNKNOWN_PARTY_NAME.to_string(),
            tax_id: None,
            address: None,
            contact: None,
        }
    }

    /// Check if this is an unknown/placeholder party
    pub fn is_unknown(&self) -> bool {
        self.id.is_nil() || self.name == UNKNOWN_PARTY_NAME
    }
}

impl Default for Party {
    fn default() -> Self {
        Self::unknown()
    }
}
