use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use image::{DynamicImage, GenericImageView};
use ndarray::Array4;
use ort::session::{Session, SessionOutputs};
use ort::value::TensorRef;

use super::{
    ExtractedTable, TableCell, TableColumn, TableExtractionResult, TableExtractorConfig, TableRow,
};
use crate::error::MlError;
use crate::ocr::{BoundingBox, OcrResult, TextRegion};

/// Table Transformer (TATR) style table extractor
pub struct TableExtractor {
    config: TableExtractorConfig,
    detection_session: Option<Mutex<Session>>,
    structure_session: Option<Mutex<Session>>,
}

impl TableExtractor {
    /// Create table extractor without models (rule-based fallback)
    pub fn new(config: TableExtractorConfig) -> Result<Self, MlError> {
        Ok(Self {
            config,
            detection_session: None,
            structure_session: None,
        })
    }

    /// Create table extractor with ONNX models
    pub fn with_models(
        config: TableExtractorConfig,
        detection_model_path: impl AsRef<Path>,
        structure_model_path: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let det_path = detection_model_path.as_ref();
        let struct_path = structure_model_path.as_ref();

        if !det_path.exists() {
            return Err(MlError::ModelNotFound(det_path.display().to_string()));
        }
        if !struct_path.exists() {
            return Err(MlError::ModelNotFound(struct_path.display().to_string()));
        }

        let detection_session = Session::builder()?.commit_from_file(det_path)?;
        let structure_session = Session::builder()?.commit_from_file(struct_path)?;

        tracing::info!(
            detection_model = %det_path.display(),
            structure_model = %struct_path.display(),
            "Loaded table extraction models"
        );

        Ok(Self {
            config,
            detection_session: Some(Mutex::new(detection_session)),
            structure_session: Some(Mutex::new(structure_session)),
        })
    }

    /// Check if models are loaded
    pub fn has_models(&self) -> bool {
        self.detection_session.is_some() && self.structure_session.is_some()
    }

    /// Extract tables from document
    pub fn extract(
        &self,
        image: &DynamicImage,
        ocr_result: &OcrResult,
    ) -> Result<TableExtractionResult, MlError> {
        let start = Instant::now();

        let tables = if self.has_models() {
            self.extract_with_models(image, ocr_result)?
        } else {
            self.extract_rule_based(image, ocr_result)?
        };

        Ok(TableExtractionResult {
            tables,
            processing_time_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Extract tables using ONNX models
    fn extract_with_models(
        &self,
        image: &DynamicImage,
        ocr_result: &OcrResult,
    ) -> Result<Vec<ExtractedTable>, MlError> {
        // Step 1: Detect tables in the image
        let table_boxes = self.detect_tables(image)?;

        if table_boxes.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: For each detected table, extract structure
        let mut tables = Vec::new();

        for (idx, (bbox, confidence)) in table_boxes.into_iter().enumerate() {
            let structure = self.extract_structure(image, &bbox)?;

            let cells = self.assign_text_to_cells(&structure, ocr_result, &bbox);

            let num_rows = structure.rows.len();
            let num_cols = structure.columns.len();

            let mut table = ExtractedTable {
                bbox,
                confidence,
                num_rows,
                num_columns: num_cols,
                cells,
                caption: None,
                table_index: idx,
            };

            // Try to find table caption
            table.caption = self.find_table_caption(ocr_result, &bbox);

            tables.push(table);
        }

        Ok(tables)
    }

    /// Detect tables in the image using detection model
    fn detect_tables(&self, image: &DynamicImage) -> Result<Vec<(BoundingBox, f32)>, MlError> {
        let mut session = self
            .detection_session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("Detection model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        let input = self.preprocess_for_detection(image)?;
        let input_tensor = TensorRef::from_array_view(&input)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let outputs = session.run(ort::inputs!["input" => input_tensor])?;

        let output = if let Some(out) = outputs.get("output") {
            out
        } else {
            &outputs[0]
        };

        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        let (img_width, img_height) = image.dimensions();
        let mut boxes = Vec::new();

        // Parse DETR-style output [batch, num_queries, 5] (x, y, w, h, confidence)
        // Shape derefs to [i64] slice

        if shape.len() >= 2 {
            let num_queries = if shape.len() == 3 { shape[1] as usize } else { shape[0] as usize };
            let stride = if shape.len() == 3 { shape[2] as usize } else { 5 };

            for i in 0..num_queries {
                let offset = if shape.len() == 3 { i * stride } else { i * stride };
                if offset + 4 >= data.len() {
                    break;
                }

                let cx = data[offset];
                let cy = data[offset + 1];
                let w = data[offset + 2];
                let h = data[offset + 3];
                let conf = if offset + 4 < data.len() { data[offset + 4] } else { 1.0 };

                if conf > self.config.detection_threshold {
                    let x: f32 = (cx - w / 2.0) * img_width as f32;
                    let y: f32 = (cy - h / 2.0) * img_height as f32;

                    boxes.push((
                        BoundingBox::new(
                            x.max(0.0),
                            y.max(0.0),
                            (w * img_width as f32).min(img_width as f32 - x),
                            (h * img_height as f32).min(img_height as f32 - y),
                        ),
                        conf,
                    ));
                }
            }
        }

        Ok(boxes)
    }

    /// Extract table structure (rows, columns, cells)
    fn extract_structure(
        &self,
        image: &DynamicImage,
        table_bbox: &BoundingBox,
    ) -> Result<TableStructure, MlError> {
        let mut session = self
            .structure_session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("Structure model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        // Crop table region
        let cropped = image.crop_imm(
            table_bbox.x as u32,
            table_bbox.y as u32,
            table_bbox.width as u32,
            table_bbox.height as u32,
        );

        let input = self.preprocess_for_structure(&cropped)?;
        let input_tensor = TensorRef::from_array_view(&input)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let outputs = session.run(ort::inputs!["input" => input_tensor])?;

        // Parse structure output (rows, columns, cells)
        self.parse_structure_output(&outputs, table_bbox)
    }

    /// Preprocess image for table detection
    fn preprocess_for_detection(&self, image: &DynamicImage) -> Result<Array4<f32>, MlError> {
        let target_size = 800u32;
        let (width, height) = image.dimensions();

        // Resize maintaining aspect ratio
        let scale = target_size as f32 / width.max(height) as f32;
        let new_w = (width as f32 * scale) as u32;
        let new_h = (height as f32 * scale) as u32;

        let resized = image.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        // Pad to target size
        let mut arr = Array4::<f32>::zeros((1, 3, target_size as usize, target_size as usize));

        // ImageNet normalization
        let mean = [0.485, 0.456, 0.406];
        let std = [0.229, 0.224, 0.225];

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            if x < target_size as usize && y < target_size as usize {
                arr[[0, 0, y, x]] = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
                arr[[0, 1, y, x]] = (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
                arr[[0, 2, y, x]] = (pixel[2] as f32 / 255.0 - mean[2]) / std[2];
            }
        }

        Ok(arr)
    }

    /// Preprocess cropped table for structure recognition
    fn preprocess_for_structure(&self, image: &DynamicImage) -> Result<Array4<f32>, MlError> {
        let target_size = 640u32;
        let resized =
            image.resize_exact(target_size, target_size, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        let mut arr = Array4::<f32>::zeros((1, 3, target_size as usize, target_size as usize));

        let mean = [0.485, 0.456, 0.406];
        let std = [0.229, 0.224, 0.225];

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            arr[[0, 0, y, x]] = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
            arr[[0, 1, y, x]] = (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
            arr[[0, 2, y, x]] = (pixel[2] as f32 / 255.0 - mean[2]) / std[2];
        }

        Ok(arr)
    }

    /// Parse structure model output
    fn parse_structure_output(
        &self,
        outputs: &SessionOutputs,
        table_bbox: &BoundingBox,
    ) -> Result<TableStructure, MlError> {
        // Get predictions for rows, columns, cells
        let mut rows = Vec::new();
        let mut columns = Vec::new();

        // Parse row predictions
        if let Some(output) = outputs.get("rows") {
            if let Ok((shape, data)) = output.try_extract_tensor::<f32>() {
                // Shape derefs to [i64] slice
                if shape.len() >= 2 {
                    let num_rows = if shape.len() == 3 { shape[1] as usize } else { shape[0] as usize };
                    let stride = if shape.len() == 3 { shape[2] as usize } else { 3 };

                    for i in 0..num_rows {
                        let offset = i * stride;
                        if offset + 2 >= data.len() {
                            break;
                        }

                        let y = data[offset];
                        let h = data[offset + 1];
                        let conf = if offset + 2 < data.len() { data[offset + 2] } else { 1.0 };

                        if conf > self.config.cell_threshold {
                            rows.push(TableRow {
                                bbox: BoundingBox::new(
                                    table_bbox.x,
                                    table_bbox.y + y * table_bbox.height,
                                    table_bbox.width,
                                    h * table_bbox.height,
                                ),
                                confidence: conf,
                                cells: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        // Parse column predictions
        if let Some(output) = outputs.get("columns") {
            if let Ok((shape, data)) = output.try_extract_tensor::<f32>() {
                // Shape derefs to [i64] slice
                if shape.len() >= 2 {
                    let num_cols = if shape.len() == 3 { shape[1] as usize } else { shape[0] as usize };
                    let stride = if shape.len() == 3 { shape[2] as usize } else { 3 };

                    for i in 0..num_cols {
                        let offset = i * stride;
                        if offset + 2 >= data.len() {
                            break;
                        }

                        let x = data[offset];
                        let w = data[offset + 1];
                        let conf = if offset + 2 < data.len() { data[offset + 2] } else { 1.0 };

                        if conf > self.config.cell_threshold {
                            columns.push(TableColumn {
                                bbox: BoundingBox::new(
                                    table_bbox.x + x * table_bbox.width,
                                    table_bbox.y,
                                    w * table_bbox.width,
                                    table_bbox.height,
                                ),
                                confidence: conf,
                                x_start: table_bbox.x + x * table_bbox.width,
                                x_end: table_bbox.x + (x + w) * table_bbox.width,
                            });
                        }
                    }
                }
            }
        }

        // Sort rows and columns by position
        rows.sort_by(|a, b| a.bbox.y.partial_cmp(&b.bbox.y).unwrap());
        columns.sort_by(|a, b| a.bbox.x.partial_cmp(&b.bbox.x).unwrap());

        Ok(TableStructure { rows, columns })
    }

    /// Assign OCR text to detected cells
    fn assign_text_to_cells(
        &self,
        structure: &TableStructure,
        ocr_result: &OcrResult,
        _table_bbox: &BoundingBox,
    ) -> Vec<Vec<TableCell>> {
        let mut cells = Vec::new();

        for (row_idx, row) in structure.rows.iter().enumerate() {
            let mut row_cells = Vec::new();

            for (col_idx, col) in structure.columns.iter().enumerate() {
                // Cell bbox is intersection of row and column
                let cell_bbox = BoundingBox::new(
                    col.x_start,
                    row.bbox.y,
                    col.width(),
                    row.bbox.height,
                );

                // Find OCR text that falls within this cell
                let text = self.find_text_in_region(&cell_bbox, ocr_result);

                row_cells.push(TableCell {
                    text,
                    bbox: cell_bbox,
                    row: row_idx,
                    column: col_idx,
                    row_span: 1,
                    col_span: 1,
                    is_header: row_idx == 0,
                    confidence: row.confidence * col.confidence,
                });
            }

            cells.push(row_cells);
        }

        cells
    }

    /// Find OCR text that falls within a bounding box
    fn find_text_in_region(&self, bbox: &BoundingBox, ocr_result: &OcrResult) -> String {
        let mut texts = Vec::new();

        for region in &ocr_result.regions {
            // Check if region overlaps with cell
            if region.bbox.intersects(bbox) {
                // Calculate overlap ratio
                let overlap = region.bbox.iou(bbox);
                if overlap > 0.3 {
                    texts.push((region.order, region.text.clone()));
                }
            }
        }

        // Sort by reading order and join
        texts.sort_by_key(|(order, _)| *order);
        texts
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Find table caption near the table
    fn find_table_caption(&self, ocr_result: &OcrResult, table_bbox: &BoundingBox) -> Option<String> {
        // Look for text just above the table that might be a caption
        let search_region = BoundingBox::new(
            table_bbox.x,
            (table_bbox.y - 50.0).max(0.0),
            table_bbox.width,
            50.0,
        );

        for region in &ocr_result.regions {
            if region.bbox.intersects(&search_region) {
                let lower = region.text.to_lowercase();
                if lower.starts_with("table")
                    || lower.contains("figure")
                    || lower.contains("exhibit")
                {
                    return Some(region.text.clone());
                }
            }
        }

        None
    }

    /// Rule-based table extraction (fallback)
    fn extract_rule_based(
        &self,
        _image: &DynamicImage,
        ocr_result: &OcrResult,
    ) -> Result<Vec<ExtractedTable>, MlError> {
        let mut tables = Vec::new();

        // Look for table-like patterns in OCR text
        let table_regions = self.find_table_regions_heuristic(ocr_result);

        for (idx, region) in table_regions.into_iter().enumerate() {
            let cells = self.parse_table_text(&region.text);

            if !cells.is_empty() && cells[0].len() >= 2 {
                let num_rows = cells.len();
                let num_cols = cells.iter().map(|r| r.len()).max().unwrap_or(0);

                tables.push(ExtractedTable {
                    bbox: region.bbox,
                    confidence: region.confidence,
                    num_rows,
                    num_columns: num_cols,
                    cells,
                    caption: None,
                    table_index: idx,
                });
            }
        }

        Ok(tables)
    }

    /// Find regions that might contain tables using heuristics
    fn find_table_regions_heuristic(&self, ocr_result: &OcrResult) -> Vec<TextRegion> {
        let mut table_regions = Vec::new();

        for region in &ocr_result.regions {
            let text = &region.text;

            // Check for table-like characteristics
            let has_tabs = text.contains('\t');
            let has_multiple_columns = text.lines().any(|line| {
                let spaces: Vec<_> = line
                    .char_indices()
                    .filter(|(_, c)| *c == ' ')
                    .map(|(i, _)| i)
                    .collect();
                // Look for consistent spacing patterns
                spaces.len() >= 2
            });
            let has_multiple_lines = text.lines().count() >= 2;
            let has_aligned_numbers = self.has_aligned_numbers(text);

            if (has_tabs || has_multiple_columns) && has_multiple_lines || has_aligned_numbers {
                table_regions.push(region.clone());
            }
        }

        table_regions
    }

    /// Check if text has aligned numbers (common in tables)
    fn has_aligned_numbers(&self, text: &str) -> bool {
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < 2 {
            return false;
        }

        // Check if numbers appear at similar positions across lines
        let number_pattern = regex::Regex::new(r"\d+\.?\d*").ok();
        if let Some(re) = number_pattern {
            let positions: Vec<Vec<usize>> = lines
                .iter()
                .map(|line| re.find_iter(line).map(|m| m.start()).collect())
                .collect();

            // Check for alignment (numbers at similar positions)
            if positions.len() >= 2 && positions.iter().all(|p| !p.is_empty()) {
                let first_positions = &positions[0];
                let aligned_count = positions[1..]
                    .iter()
                    .filter(|p| {
                        p.iter()
                            .any(|&pos| first_positions.iter().any(|&fp| (pos as i32 - fp as i32).abs() < 5))
                    })
                    .count();

                return aligned_count >= positions.len() / 2;
            }
        }

        false
    }

    /// Parse table-like text into cells
    fn parse_table_text(&self, text: &str) -> Vec<Vec<TableCell>> {
        let mut cells = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        // Detect column separators
        let separator = if text.contains('\t') {
            "\t"
        } else {
            // Use multiple spaces as separator
            "  "
        };

        for (row_idx, line) in lines.iter().enumerate() {
            let columns: Vec<&str> = line.split(separator).filter(|s| !s.is_empty()).collect();

            if columns.is_empty() {
                continue;
            }

            let row_cells: Vec<TableCell> = columns
                .iter()
                .enumerate()
                .map(|(col_idx, &text)| {
                    TableCell::new(
                        text.trim().to_string(),
                        BoundingBox::new(0.0, 0.0, 0.0, 0.0), // No position info in rule-based mode
                        row_idx,
                        col_idx,
                    )
                })
                .collect();

            cells.push(row_cells);
        }

        cells
    }
}

/// Internal structure representation
struct TableStructure {
    rows: Vec<TableRow>,
    columns: Vec<TableColumn>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_extractor_without_models() {
        let config = TableExtractorConfig::default();
        let extractor = TableExtractor::new(config).unwrap();
        assert!(!extractor.has_models());
    }

    #[test]
    fn test_extracted_table_to_markdown() {
        let mut table = ExtractedTable::new(BoundingBox::new(0.0, 0.0, 100.0, 100.0), 0.9);
        table.cells = vec![
            vec![
                TableCell::new("Header 1".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 0, 0),
                TableCell::new("Header 2".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 0, 1),
            ],
            vec![
                TableCell::new("Data 1".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 1, 0),
                TableCell::new("Data 2".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 1, 1),
            ],
        ];
        table.num_rows = 2;
        table.num_columns = 2;

        let md = table.to_markdown();
        assert!(md.contains("Header 1"));
        assert!(md.contains("| --- |"));
        assert!(md.contains("Data 1"));
    }

    #[test]
    fn test_extracted_table_to_csv() {
        let mut table = ExtractedTable::new(BoundingBox::new(0.0, 0.0, 100.0, 100.0), 0.9);
        table.cells = vec![
            vec![
                TableCell::new("Name".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 0, 0),
                TableCell::new("Value".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 0, 1),
            ],
            vec![
                TableCell::new("Item, 1".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 1, 0),
                TableCell::new("100".to_string(), BoundingBox::new(0.0, 0.0, 0.0, 0.0), 1, 1),
            ],
        ];

        let csv = table.to_csv();
        assert!(csv.contains("Name,Value"));
        assert!(csv.contains("\"Item, 1\"")); // Quoted due to comma
    }
}
