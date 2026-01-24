use serde::{Deserialize, Serialize};

use crate::ocr::BoundingBox;

/// Configuration for table extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableExtractorConfig {
    /// Path to table detection ONNX model
    pub detection_model_path: Option<String>,

    /// Path to table structure recognition model
    pub structure_model_path: Option<String>,

    /// Confidence threshold for table detection
    pub detection_threshold: f32,

    /// Confidence threshold for cell detection
    pub cell_threshold: f32,

    /// Enable GPU acceleration
    pub gpu_enabled: bool,
}

impl Default for TableExtractorConfig {
    fn default() -> Self {
        Self {
            detection_model_path: None,
            structure_model_path: None,
            detection_threshold: 0.7,
            cell_threshold: 0.5,
            gpu_enabled: false,
        }
    }
}

/// A detected table in a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedTable {
    /// Table bounding box
    pub bbox: BoundingBox,

    /// Table confidence score
    pub confidence: f32,

    /// Number of rows
    pub num_rows: usize,

    /// Number of columns
    pub num_columns: usize,

    /// Table cells (row-major order)
    pub cells: Vec<Vec<TableCell>>,

    /// Optional table caption
    pub caption: Option<String>,

    /// Table index on the page
    pub table_index: usize,
}

impl ExtractedTable {
    pub fn new(bbox: BoundingBox, confidence: f32) -> Self {
        Self {
            bbox,
            confidence,
            num_rows: 0,
            num_columns: 0,
            cells: Vec::new(),
            caption: None,
            table_index: 0,
        }
    }

    /// Get cell at specific row and column
    pub fn get_cell(&self, row: usize, col: usize) -> Option<&TableCell> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    /// Get header row (first row)
    pub fn header_row(&self) -> Option<&Vec<TableCell>> {
        self.cells.first()
    }

    /// Get data rows (all except first)
    pub fn data_rows(&self) -> &[Vec<TableCell>] {
        if self.cells.len() > 1 {
            &self.cells[1..]
        } else {
            &[]
        }
    }

    /// Convert table to markdown format
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        for (row_idx, row) in self.cells.iter().enumerate() {
            let row_text: Vec<String> = row.iter().map(|c| c.text.clone()).collect();
            md.push_str("| ");
            md.push_str(&row_text.join(" | "));
            md.push_str(" |\n");

            // Add separator after header
            if row_idx == 0 {
                let separator: Vec<String> = row.iter().map(|_| "---".to_string()).collect();
                md.push_str("| ");
                md.push_str(&separator.join(" | "));
                md.push_str(" |\n");
            }
        }

        md
    }

    /// Convert table to CSV format
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();

        for row in &self.cells {
            let row_text: Vec<String> = row
                .iter()
                .map(|c| {
                    let escaped = c.text.replace('"', "\"\"");
                    if escaped.contains(',') || escaped.contains('\n') || escaped.contains('"') {
                        format!("\"{}\"", escaped)
                    } else {
                        escaped
                    }
                })
                .collect();
            csv.push_str(&row_text.join(","));
            csv.push('\n');
        }

        csv
    }
}

/// A cell within a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    /// Cell text content
    pub text: String,

    /// Cell bounding box
    pub bbox: BoundingBox,

    /// Row index (0-based)
    pub row: usize,

    /// Column index (0-based)
    pub column: usize,

    /// Row span (for merged cells)
    pub row_span: usize,

    /// Column span (for merged cells)
    pub col_span: usize,

    /// Whether this is a header cell
    pub is_header: bool,

    /// Cell confidence score
    pub confidence: f32,
}

impl TableCell {
    pub fn new(text: String, bbox: BoundingBox, row: usize, column: usize) -> Self {
        Self {
            text,
            bbox,
            row,
            column,
            row_span: 1,
            col_span: 1,
            is_header: row == 0,
            confidence: 1.0,
        }
    }

    /// Check if this cell spans multiple rows or columns
    pub fn is_merged(&self) -> bool {
        self.row_span > 1 || self.col_span > 1
    }
}

/// Result of table extraction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableExtractionResult {
    /// Extracted tables
    pub tables: Vec<ExtractedTable>,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Row detected in a table
#[derive(Debug, Clone)]
pub struct TableRow {
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub cells: Vec<TableCell>,
}

/// Column detected in a table
#[derive(Debug, Clone)]
pub struct TableColumn {
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub x_start: f32,
    pub x_end: f32,
}

impl TableColumn {
    pub fn width(&self) -> f32 {
        self.x_end - self.x_start
    }
}
