#!/usr/bin/env python3
"""Fine-tune GLiNER medium model on production extraction logs.

Reads JSONL extraction logs produced by fen-ingestion's ExtractionLogger,
converts them to GLiNER training format, fine-tunes the model, and exports
to ONNX for deployment.

Usage:
    python scripts/finetune-gliner.py \
        --logs-dir ./extraction_logs \
        --output-dir ./models \
        --base-model urchade/gliner_medium-v2.1 \
        [--epochs 3] [--batch-size 8] [--lr 1e-5] [--min-samples 100]

The script is safe to re-run — it reads all available JSONL files and
trains on the full dataset. The output model replaces the previous
fine-tuned checkpoint.

Requirements:
    pip install gliner torch optimum[exporters] transformers
"""

import argparse
import json
import sys
from pathlib import Path


def load_extraction_logs(logs_dir: Path, min_confidence: float) -> list[dict]:
    """Load and parse JSONL extraction log files.

    Each line is an ExtractionEvent:
    {
        "document_id": "...",
        "text": "...",
        "entities": [{"label": "...", "text": "...", "char_start": N, "char_end": N}],
        "source": "layout_lmv3" | "gliner_medium" | ...,
        "confidence": 0.85,
        "timestamp": "..."
    }
    """
    events = []
    jsonl_files = sorted(logs_dir.glob("extractions_*.jsonl"))

    if not jsonl_files:
        print(f"No extraction log files found in {logs_dir}")
        return events

    for filepath in jsonl_files:
        print(f"  Reading {filepath.name}...")
        with open(filepath) as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    event = json.loads(line)
                    if event.get("confidence", 0) >= min_confidence:
                        events.append(event)
                except json.JSONDecodeError as e:
                    print(f"    [WARN] {filepath.name}:{line_num}: {e}")

    print(f"  Loaded {len(events)} high-confidence extraction events")
    return events


def convert_to_gliner_format(events: list[dict]) -> list[dict]:
    """Convert extraction events to GLiNER training format.

    GLiNER expects:
    [
        {
            "tokenized_text": ["word1", "word2", ...],
            "ner": [[start_word, end_word, "label"], ...]
        },
        ...
    ]

    We convert character offsets to word-level indices.
    """
    training_data = []

    for event in events:
        text = event.get("text", "")
        entities = event.get("entities", [])

        if not text or not entities:
            continue

        # Tokenize by whitespace (word-level)
        words = text.split()
        if not words:
            continue

        # Build character offset → word index mapping
        word_starts = []
        word_ends = []
        pos = 0
        for word in words:
            start = text.find(word, pos)
            if start == -1:
                start = pos
            word_starts.append(start)
            word_ends.append(start + len(word))
            pos = start + len(word)

        # Convert char offsets to word indices
        ner_annotations = []
        for entity in entities:
            char_start = entity.get("char_start", 0)
            char_end = entity.get("char_end", 0)
            label = entity.get("label", "")

            if not label or char_start >= char_end:
                continue

            # Find start word
            start_word = None
            for i, ws in enumerate(word_starts):
                if ws <= char_start < word_ends[i]:
                    start_word = i
                    break
                if ws > char_start:
                    start_word = max(0, i - 1)
                    break

            # Find end word
            end_word = None
            for i, we in enumerate(word_ends):
                if we >= char_end:
                    end_word = i
                    break

            if start_word is not None and end_word is not None:
                ner_annotations.append([start_word, end_word, label])

        if ner_annotations:
            training_data.append({
                "tokenized_text": words,
                "ner": ner_annotations,
            })

    print(f"  Converted {len(training_data)} samples to GLiNER format")
    return training_data


def finetune_gliner(
    training_data: list[dict],
    base_model: str,
    output_dir: Path,
    epochs: int,
    batch_size: int,
    learning_rate: float,
) -> Path:
    """Fine-tune GLiNER model and return path to saved checkpoint."""
    try:
        from gliner import GLiNER
    except ImportError:
        print("ERROR: 'gliner' package not installed. Run: pip install gliner")
        sys.exit(1)

    print(f"\n  Loading base model: {base_model}")
    model = GLiNER.from_pretrained(base_model)

    # Collect all unique labels from training data
    all_labels = set()
    for sample in training_data:
        for annotation in sample.get("ner", []):
            all_labels.add(annotation[2])

    print(f"  Labels: {sorted(all_labels)}")
    print(f"  Training samples: {len(training_data)}")
    print(f"  Epochs: {epochs}, Batch size: {batch_size}, LR: {learning_rate}")

    # Train
    train_config = {
        "num_steps": epochs * (len(training_data) // batch_size + 1),
        "train_batch_size": batch_size,
        "learning_rate": learning_rate,
        "weight_decay": 0.01,
        "warmup_ratio": 0.1,
        "log_dir": str(output_dir / "training_logs"),
    }

    model.train(training_data, **train_config)

    # Save PyTorch checkpoint
    checkpoint_dir = output_dir / "gliner_finetuned_checkpoint"
    checkpoint_dir.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(checkpoint_dir)
    print(f"  Saved PyTorch checkpoint to {checkpoint_dir}")

    return checkpoint_dir


def export_to_onnx(checkpoint_dir: Path, output_path: Path) -> None:
    """Export fine-tuned model to ONNX via optimum."""
    import subprocess

    print(f"\n  Exporting to ONNX: {output_path}")

    import tempfile
    with tempfile.TemporaryDirectory() as tmp_dir:
        cmd = [
            sys.executable, "-m", "optimum.exporters.onnx",
            "--model", str(checkpoint_dir),
            "--task", "token-classification",
            "--opset", "17",
            str(tmp_dir),
        ]

        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            err = result.stderr.strip()
            if err:
                print(f"  stderr: {err[-500:]}")
            raise RuntimeError(f"ONNX export failed (exit {result.returncode})")

        # Find and copy the ONNX file
        import shutil
        onnx_files = list(Path(tmp_dir).glob("*.onnx"))
        if not onnx_files:
            onnx_files = list(Path(tmp_dir).glob("**/*.onnx"))
        if not onnx_files:
            raise FileNotFoundError("No ONNX file produced by export")

        shutil.copy2(onnx_files[0], output_path)

    size_mb = output_path.stat().st_size / (1024 * 1024)
    print(f"  Exported: {output_path} ({size_mb:.1f} MB)")


def main():
    parser = argparse.ArgumentParser(
        description="Fine-tune GLiNER on production extraction logs"
    )
    parser.add_argument(
        "--logs-dir",
        type=Path,
        default=Path("./extraction_logs"),
        help="Directory containing JSONL extraction logs",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("./models"),
        help="Directory to save fine-tuned model",
    )
    parser.add_argument(
        "--base-model",
        type=str,
        default="urchade/gliner_medium-v2.1",
        help="Base GLiNER model to fine-tune",
    )
    parser.add_argument(
        "--epochs",
        type=int,
        default=3,
        help="Number of training epochs",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=8,
        help="Training batch size",
    )
    parser.add_argument(
        "--lr",
        type=float,
        default=1e-5,
        help="Learning rate",
    )
    parser.add_argument(
        "--min-samples",
        type=int,
        default=100,
        help="Minimum extraction events required to proceed with fine-tuning",
    )
    parser.add_argument(
        "--min-confidence",
        type=float,
        default=0.7,
        help="Minimum confidence threshold for training data",
    )
    args = parser.parse_args()

    print("=" * 60)
    print("GLiNER Fine-Tuning Pipeline")
    print("=" * 60)

    # Step 1: Load extraction logs
    print("\n[1/4] Loading extraction logs...")
    events = load_extraction_logs(args.logs_dir, args.min_confidence)

    if len(events) < args.min_samples:
        print(f"\nInsufficient data: {len(events)} events < {args.min_samples} minimum")
        print("Collect more production extractions before fine-tuning.")
        sys.exit(0)

    # Step 2: Convert to GLiNER format
    print("\n[2/4] Converting to GLiNER training format...")
    training_data = convert_to_gliner_format(events)

    if not training_data:
        print("No valid training samples produced. Check extraction log format.")
        sys.exit(1)

    # Step 3: Fine-tune
    print(f"\n[3/4] Fine-tuning {args.base_model}...")
    checkpoint_dir = finetune_gliner(
        training_data,
        args.base_model,
        args.output_dir,
        args.epochs,
        args.batch_size,
        args.lr,
    )

    # Step 4: Export to ONNX
    print("\n[4/4] Exporting to ONNX...")
    output_onnx = args.output_dir / "gliner_medium_finetuned.onnx"
    export_to_onnx(checkpoint_dir, output_onnx)

    print("\n" + "=" * 60)
    print("Fine-tuning complete!")
    print("=" * 60)
    print(f"  Training samples: {len(training_data)}")
    print(f"  ONNX model:       {output_onnx}")
    print(f"\nTo deploy, set finetuned_model_path in GlinerModelConfig:")
    print(f'  finetuned_model_path: Some("{output_onnx.name}".to_string())')
    print()


if __name__ == "__main__":
    main()
