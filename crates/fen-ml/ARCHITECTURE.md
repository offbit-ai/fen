# fen-ml Architecture

Machine learning components for document intelligence. All inference runs locally via ONNX Runtime — no external API calls, no cloud dependencies.

## Pipeline Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    DocumentIntelligence                         │
│                                                                 │
│  ┌─────────┐   ┌──────────┐   ┌───────┐   ┌───────────┐       │
│  │   OCR   │──→│ LayoutLMv3│──→│ TATR  │   │ Embedding │       │
│  │ (PP-OCR)│   │          │   │       │   │(MiniLM-L6)│       │
│  └─────────┘   └──────────┘   └───────┘   └───────────┘       │
│       │              │             │              │             │
│   text regions   layout regions  tables    384-dim vectors     │
│       │              │             │              │             │
│       └──────────────┴─────────────┴──────────────┘             │
│                         │                                       │
│                  ProcessedDocument                               │
│                                                                 │
│  ┌──────────┐   ┌──────────┐                                   │
│  │  GLiNER  │   │  Donut   │  ← text-only / image-only paths  │
│  │ (NER)    │   │(vision)  │                                   │
│  └──────────┘   └──────────┘                                   │
└─────────────────────────────────────────────────────────────────┘
```

**Primary path** (image input): OCR → LayoutLMv3 → TATR → Embedding → ProcessedDocument

**Text-only path**: GLiNER extracts entities directly from text (no rendering needed)

**Vision fallback**: Donut processes image → structured JSON (no OCR needed)

## Model Inventory

| Model | File | Size | Source | Input | Output |
|-------|------|------|--------|-------|--------|
| OCR Detection | `ocr_detection.onnx` | 2.4 MB | PaddleOCR v4 DBNet++ | Image [1,3,H,W] | Probability map [1,H,W] |
| OCR Recognition | `ocr_recognition.onnx` | 8.9 MB | PaddleOCR v4 SVTR-LCNetV2 | Crop [1,3,48,W] | Logits [1,seq,vocab] |
| Layout Analysis | `layout_model.onnx` | 478 MB | LayoutLMv3-base | Tokens + bbox + image | Labels [1,seq,num_labels] |
| Layout Tokenizer | `layout_tokenizer.json` | 3.6 MB | LayoutLMv3-base | — | — |
| Table Detection | `table_detection.onnx` | 116 MB | DETR table-transformer | Image [1,3,800,800] | Boxes [1,Q,5] |
| Table Structure | `table_structure.onnx` | 116 MB | DETR table-structure | Table crop | Row/col boxes |
| Embedding | `embedding_model.onnx` | 86 MB | all-MiniLM-L6-v2 | Tokens [B,seq] | Embeddings [B,384] |
| Embedding Tokenizer | `tokenizer.json` | 0.7 MB | all-MiniLM-L6-v2 | — | — |
| GLiNER Medium | `gliner_medium.onnx` | ~200 MB | gliner_medium-v2.1 | Tokens + spans | Logits [spans,labels] |
| GLiNER Large | `gliner_large.onnx` | ~400 MB | gliner_large-v2.1 | Tokens + spans | Logits [spans,labels] |
| Donut Encoder | `donut_encoder.onnx` | ~200 MB | donut-base-finetuned-cord-v2 | Image [1,3,H,W] | Hidden states |
| Donut Decoder | `donut_decoder.onnx` | ~200 MB | donut-base-finetuned-cord-v2 | Tokens + encoder out | Next token logits |

Total memory footprint: ~1.6 GB (all models loaded).

## ONNX Integration Pattern

Every model follows the same structure:

```rust
pub struct ModelConfig {
    pub model_path: Option<String>,    // None = rule-based fallback
    // ... model-specific config
}

pub struct Model {
    config: ModelConfig,
    session: Option<Mutex<Session>>,   // Thread-safe ONNX session
    tokenizer: Option<Tokenizer>,      // If text input needed
}

impl Model {
    pub fn new(config) -> Result<Self, MlError>;           // No model (fallback)
    pub fn with_models(config, paths) -> Result<Self, MlError>;  // Load ONNX
    pub fn has_model(&self) -> bool;                       // Check if loaded
}
```

**Key patterns:**
- `Session::builder()?.commit_from_file(path)` to load ONNX
- `Mutex<Session>` for thread-safe inference
- `ort::inputs![]` macro for tensor input
- `try_extract_tensor::<f32>()` for output extraction
- Every model degrades gracefully to rule-based when no ONNX file is present

## Component Details

### OCR (PaddleOCR v4)

Two-stage pipeline:
1. **Detection** (DBNet++): Input image → probability map → connected component analysis → text region bounding boxes
2. **Recognition** (SVTR-LCNetV2 + CTC): Crop each region → character sequence via CTC greedy/beam decoding

Supports multi-language via per-language recognition models + character dictionaries. Detection model is shared across languages.

Preprocessing options: deskew, denoise, contrast enhancement, binarization (`PreprocessingConfig`).

### LayoutLMv3

Multi-modal transformer combining text tokens, bounding boxes, and image features. Performs token-level classification into layout labels (Text, Title, List, Table, Figure) and extracts:
- **Layout regions** with labels and confidence
- **Named entities** (Date, Amount, InvoiceNumber, Organization, Email)
- **Key-value pairs** from form-like structures

Input: tokenized text with word-level bounding boxes (normalized 0-1000) + image (224x224 ImageNet-normalized).

Falls back to rule-based heuristics when no model is loaded (position/content-based region classification, regex-based entity extraction).

### TATR (Table Transformer)

Two-stage DETR-based pipeline:
1. **Detection**: Finds table bounding boxes in document image
2. **Structure Recognition**: Identifies rows, columns, and spanning cells within detected tables

Cells are assigned OCR text via R-tree spatial matching. Supports export to markdown, CSV, and HTML.

### Sentence Embeddings (all-MiniLM-L6-v2)

Produces 384-dimensional L2-normalized embeddings for:
- Full document text
- Individual sections (by layout region)
- Extracted entities

Uses mean pooling over token embeddings. LRU cache (1024 entries) for repeated text. Supports batch inference.

### GLiNER (Zero-Shot NER)

Two-tier extraction architecture for dynamic entity recognition:

```
┌────────────────────────────────────────────────────┐
│                   Input Text                        │
│                   + Labels                          │
│                                                    │
│  ┌──────────────┐                                  │
│  │ Medium Model  │───→ confidence satisfactory? ──→ Return │
│  │ (~60ms)       │         │ NO                    │
│  └──────────────┘          ▼                       │
│                   ┌──────────────┐                 │
│                   │ Large Model   │───→ Return     │
│                   │ (~200ms)      │                │
│                   └──────────────┘                 │
└────────────────────────────────────────────────────┘
```

**Preprocessing**: Text → word tokenization → label tokenization → span enumeration → input tensors (`input_ids`, `attention_mask`, `token_type_ids`, `words_mask`, `text_lengths`, `span_idx`, `span_mask`)

**Postprocessing**: Logits → sigmoid → threshold filter → Non-Maximum Suppression (NMS per label, word-level IoU) → character offset mapping

**Confidence evaluation** (`ExtractionConfidence`):
- `mean_entity_score`: average confidence across entities
- `completeness`: fraction of expected labels found
- `min_entity_score`: weakest entity (hard gate at 0.3)
- `overall`: composite score (0.4×mean + 0.3×completeness + 0.2×min + 0.1×low_conf_penalty)

**Escalation**: Medium → Large when `overall < 0.65` or `completeness < 0.6`

**Zero-shot**: Labels are passed at inference time, not baked into the model. Fine-tuning improves span detection quality, not the label vocabulary.

### Donut (Vision Encoder-Decoder)

End-to-end document understanding: image → structured JSON without OCR.

**Architecture**: Swin Transformer encoder + BART decoder. Autoregressive decoding loop:
1. Encode image → hidden states
2. Seed decoder with task-specific prompt token
3. Loop: previous token + encoder states → next token logits → greedy argmax
4. Stop at EOS or max length
5. Parse output as JSON

**Use case**: Fallback when OCR → LayoutLMv3 produces low confidence (<0.5). Particularly effective for receipts and structured forms where layout is consistent.

## Extraction Fallback Chain

When processing documents through the ingestion pipeline, entity extraction follows a 5-tier fallback per field:

```
LayoutLMv3 entities → KV pairs → GLiNER → Rule Engine → Regex
```

GLiNER runs lazily (only when LayoutLMv3 produces < 3 entities). The rule engine uses contextual line scanning (label + value proximity). Regex is the last resort.

When primary pipeline confidence < 0.5, Donut runs as a parallel vision path and its results are merged with the primary extraction.

## Fine-Tuning Loop

GLiNER can be fine-tuned on production data via a self-improving feedback loop:

```
┌──────────────────────────────────────────────────────────────────┐
│                    Production Pipeline                            │
│                                                                  │
│  PDF ──→ OCR ──→ LayoutLMv3 ──→ GLiNER ──→ Rule Engine ──→ Regex │
│                                                                  │
│          ┌─────────────────────────────┐                         │
│          │  Confidence > 0.7?          │                         │
│          │  YES → ExtractionLogger     │                         │
│          └────────────┬────────────────┘                         │
└───────────────────────┼──────────────────────────────────────────┘
                        │
                        ▼
┌───────────────────────────────────────────┐
│  extraction_logs/extractions_YYYYMMDD.jsonl │
│                                           │
│  {"text": "...",                           │
│   "entities": [                           │
│     {"label": "invoice_number",           │
│      "text": "INV-001",                   │
│      "char_start": 11, "char_end": 18}    │
│   ],                                      │
│   "source": "layout_lmv3",               │
│   "confidence": 0.85}                     │
└─────────────────┬─────────────────────────┘
                  │
                  ▼  scripts/finetune-gliner.py
┌─────────────────────────────────────────┐
│  1. Load JSONL logs (min 100 samples)   │
│  2. Convert char offsets → word indices  │
│  3. Fine-tune gliner_medium-v2.1        │
│     (GLiNER library, PyTorch)           │
│  4. Export → ONNX via optimum           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│  models/gliner_medium_finetuned.onnx    │
└─────────────────┬───────────────────────┘
                  │
                  ▼  GlinerModelConfig.finetuned_model_path
┌─────────────────────────────────────────┐
│  GlinerExtractor loads fine-tuned model │
│  via effective_model_path()             │
│  (prefers finetuned over base)          │
└─────────────────┬───────────────────────┘
                  │
                  ▼  Better span detection
         ┌────────────────┐
         │ Fewer fallbacks │──→ Back to Production Pipeline
         │ Higher accuracy │     (cycle repeats)
         └────────────────┘
```

**Key design decisions:**

- **Zero-shot stays dynamic**: Labels are passed at inference time, not baked in. Fine-tuning improves *span detection quality* (where entities start/end), not the label vocabulary.
- **Provenance tracking**: Every logged event records which tier (`LayoutLMv3`, `GlinerMedium`, `GlinerLarge`, `RuleEngine`, `Regex`) produced the extraction, enabling analysis of which tiers benefit most from fine-tuning.
- **Confidence gating**: Only extractions above 0.7 confidence become training data, preventing noisy low-quality extractions from degrading the model.
- **Safe deployment**: `finetuned_model_path` is optional — the base model is always the fallback if no fine-tuned checkpoint exists.

## Model Management

Models are managed via `scripts/download-models.py`:

```bash
./scripts/setup-models.sh              # Download all models
./scripts/setup-models.sh --verify     # Check SHA256 integrity
./scripts/setup-models.sh --force      # Re-download all
./scripts/setup-models.sh --languages en,ch,ja  # Add OCR languages
```

Models are tracked in `models/manifest.json` with SHA256 checksums. The script supports three download methods:
- `huggingface_file`: Direct file download from HuggingFace Hub
- `optimum_export`: Export PyTorch model to ONNX via Hugging Face Optimum
- `tokenizer`: Download and save tokenizer via AutoTokenizer

## Key Source Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | `DocumentIntelligence` orchestrator, `ProcessedDocument` |
| `src/ocr/engine.rs` | PaddleOCR detection + recognition |
| `src/ocr/preprocessing.rs` | Image preprocessing (deskew, denoise, etc.) |
| `src/layout/model.rs` | LayoutLMv3 analysis + rule-based fallback |
| `src/table/extractor.rs` | DETR table detection + structure |
| `src/embedding/model.rs` | MiniLM embeddings with LRU cache |
| `src/gliner/extractor.rs` | Two-tier GLiNER orchestration |
| `src/gliner/preprocessor.rs` | Text → token + span tensors |
| `src/gliner/postprocessor.rs` | Logits → entities (sigmoid, NMS) |
| `src/gliner/confidence.rs` | Confidence evaluation + escalation |
| `src/donut/model.rs` | Donut encoder-decoder ONNX inference |
| `src/donut/decoder.rs` | Autoregressive token generation |
| `src/error.rs` | `MlError` type |
