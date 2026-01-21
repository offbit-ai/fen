use serde::{Deserialize, Serialize};

use crate::ocr::BoundingBox;

/// Configuration for layout understanding model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutModelConfig {
    /// Path to ONNX model
    pub model_path: Option<String>,

    /// Maximum sequence length
    pub max_seq_length: usize,

    /// Image size for vision encoder
    pub image_size: u32,

    /// Number of threads for CPU inference
    pub num_threads: usize,

    /// Enable GPU acceleration
    pub gpu_enabled: bool,
}

impl Default for LayoutModelConfig {
    fn default() -> Self {
        Self {
            model_path: None,
            max_seq_length: 512,
            image_size: 224,
            num_threads: 4,
            gpu_enabled: false,
        }
    }
}

/// Document region type (semantic label)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayoutLabel {
    /// Document title
    Title,
    /// Text content
    Text,
    /// List item
    List,
    /// Table region
    Table,
    /// Figure/image
    Figure,
    /// Header region
    Header,
    /// Footer region
    Footer,
    /// Page number
    PageNumber,
    /// Caption text
    Caption,
    /// Form field
    FormField,
    /// Signature area
    Signature,
    /// Logo/stamp
    Logo,
    /// Other/unknown
    Other,
}

impl LayoutLabel {
    /// Get label from model output index
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => LayoutLabel::Title,
            1 => LayoutLabel::Text,
            2 => LayoutLabel::List,
            3 => LayoutLabel::Table,
            4 => LayoutLabel::Figure,
            5 => LayoutLabel::Header,
            6 => LayoutLabel::Footer,
            7 => LayoutLabel::PageNumber,
            8 => LayoutLabel::Caption,
            9 => LayoutLabel::FormField,
            10 => LayoutLabel::Signature,
            11 => LayoutLabel::Logo,
            _ => LayoutLabel::Other,
        }
    }

    /// Get model output index for label
    pub fn to_index(&self) -> usize {
        match self {
            LayoutLabel::Title => 0,
            LayoutLabel::Text => 1,
            LayoutLabel::List => 2,
            LayoutLabel::Table => 3,
            LayoutLabel::Figure => 4,
            LayoutLabel::Header => 5,
            LayoutLabel::Footer => 6,
            LayoutLabel::PageNumber => 7,
            LayoutLabel::Caption => 8,
            LayoutLabel::FormField => 9,
            LayoutLabel::Signature => 10,
            LayoutLabel::Logo => 11,
            LayoutLabel::Other => 12,
        }
    }
}

/// A layout region in a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutRegion {
    /// Region label
    pub label: LayoutLabel,

    /// Bounding box
    pub bbox: BoundingBox,

    /// Confidence score
    pub confidence: f32,

    /// Text content in this region
    pub text: String,

    /// Child regions (for nested structures)
    pub children: Vec<LayoutRegion>,
}

impl LayoutRegion {
    pub fn new(label: LayoutLabel, bbox: BoundingBox, confidence: f32, text: String) -> Self {
        Self {
            label,
            bbox,
            confidence,
            text,
            children: Vec::new(),
        }
    }
}

/// Named entity type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    /// Invoice number
    InvoiceNumber,
    /// Date (invoice date, due date, etc.)
    Date,
    /// Money amount
    Amount,
    /// Person name
    PersonName,
    /// Organization name
    Organization,
    /// Address
    Address,
    /// Phone number
    Phone,
    /// Email address
    Email,
    /// Tax ID / VAT number
    TaxId,
    /// Product/service description
    ProductDescription,
    /// Quantity
    Quantity,
    /// Unit price
    UnitPrice,
    /// PO number
    PoNumber,
    /// Bank account
    BankAccount,
    /// Contract number
    ContractNumber,
    /// Other
    Other,
}

impl EntityType {
    pub fn from_label(label: &str) -> Self {
        match label.to_lowercase().as_str() {
            "invoice_number" | "invoice_no" | "inv_num" => EntityType::InvoiceNumber,
            "date" | "invoice_date" | "due_date" => EntityType::Date,
            "amount" | "total" | "subtotal" | "tax" => EntityType::Amount,
            "person" | "name" | "person_name" => EntityType::PersonName,
            "org" | "organization" | "company" | "vendor" => EntityType::Organization,
            "address" | "addr" => EntityType::Address,
            "phone" | "tel" => EntityType::Phone,
            "email" => EntityType::Email,
            "tax_id" | "vat" | "ein" => EntityType::TaxId,
            "product" | "description" | "item" => EntityType::ProductDescription,
            "quantity" | "qty" => EntityType::Quantity,
            "unit_price" | "price" => EntityType::UnitPrice,
            "po_number" | "po" => EntityType::PoNumber,
            "bank" | "account" | "iban" => EntityType::BankAccount,
            "contract_number" | "contract_no" => EntityType::ContractNumber,
            _ => EntityType::Other,
        }
    }
}

/// A named entity extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedEntity {
    /// Entity type
    pub entity_type: EntityType,

    /// Entity text value
    pub value: String,

    /// Confidence score
    pub confidence: f32,

    /// Bounding box (if available)
    pub bbox: Option<BoundingBox>,

    /// Character start position in text
    pub start_pos: usize,

    /// Character end position in text
    pub end_pos: usize,
}

impl NamedEntity {
    pub fn new(
        entity_type: EntityType,
        value: String,
        confidence: f32,
        start_pos: usize,
        end_pos: usize,
    ) -> Self {
        Self {
            entity_type,
            value,
            confidence,
            bbox: None,
            start_pos,
            end_pos,
        }
    }

    pub fn with_bbox(mut self, bbox: BoundingBox) -> Self {
        self.bbox = Some(bbox);
        self
    }
}

/// Key-value pair extracted from document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValuePair {
    /// Key text
    pub key: String,

    /// Value text
    pub value: String,

    /// Confidence score
    pub confidence: f32,

    /// Key bounding box
    pub key_bbox: Option<BoundingBox>,

    /// Value bounding box
    pub value_bbox: Option<BoundingBox>,
}

impl KeyValuePair {
    pub fn new(key: String, value: String, confidence: f32) -> Self {
        Self {
            key,
            value,
            confidence,
            key_bbox: None,
            value_bbox: None,
        }
    }
}

/// Result of layout analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutResult {
    /// Document regions
    pub regions: Vec<LayoutRegion>,

    /// Extracted entities
    pub entities: Vec<NamedEntity>,

    /// Key-value pairs
    pub key_value_pairs: Vec<KeyValuePair>,

    /// Full document text (reading order)
    pub text: String,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

impl Default for LayoutResult {
    fn default() -> Self {
        Self {
            regions: Vec::new(),
            entities: Vec::new(),
            key_value_pairs: Vec::new(),
            text: String::new(),
            processing_time_ms: 0,
        }
    }
}
