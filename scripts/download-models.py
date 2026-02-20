#!/usr/bin/env python3
"""Download and convert ML models for Fen document intelligence pipeline.

Models are downloaded from HuggingFace Hub and converted to ONNX format
using the Optimum library. This script is idempotent -- it skips models
that already exist in the target directory.

Usage:
    python scripts/download-models.py [--models-dir ./models] [--force] [--dry-run] [--verify]
"""

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path


# ---------------------------------------------------------------------------
# Model definitions -- each entry specifies what to download and how
# ---------------------------------------------------------------------------

MODELS = [
    # OCR Detection (PaddleOCR v4, language-agnostic DBNet++)
    {
        "name": "ocr_detection",
        "output": "ocr_detection.onnx",
        "source": "deepghs/paddleocr",
        "method": "huggingface_file",
        "repo_id": "deepghs/paddleocr",
        "filename": "det/ch_PP-OCRv4_det/model.onnx",
        "description": "PaddleOCR v4 text detection model (DBNet++, all languages)",
    },
    # OCR Recognition (PaddleOCR v4 English, SVTR-LCNetV2 + CTC)
    {
        "name": "ocr_recognition",
        "output": "ocr_recognition.onnx",
        "source": "deepghs/paddleocr",
        "method": "huggingface_file",
        "repo_id": "deepghs/paddleocr",
        "filename": "rec/en_PP-OCRv4_rec/model.onnx",
        "description": "PaddleOCR v4 English text recognition model (SVTR-LCNetV2 + CTC)",
    },
    # OCR English dictionary (PP-OCRv4)
    {
        "name": "ocr_dict_en",
        "output": "ocr_dicts/en_dict.txt",
        "source": "deepghs/paddleocr",
        "method": "huggingface_file",
        "repo_id": "deepghs/paddleocr",
        "filename": "rec/en_PP-OCRv4_rec/dict.txt",
        "description": "PP-OCRv4 English character dictionary",
    },
    # Layout understanding (LayoutLMv3 -- requires optimum export)
    {
        "name": "layout_model",
        "output": "layout_model.onnx",
        "source": "microsoft/layoutlmv3-base",
        "method": "optimum_export",
        "repo_id": "microsoft/layoutlmv3-base",
        "task": "token-classification",
        "opset": 17,
        "description": "LayoutLMv3 document layout analysis model",
    },
    # Layout tokenizer
    {
        "name": "layout_tokenizer",
        "output": "layout_tokenizer.json",
        "source": "microsoft/layoutlmv3-base",
        "method": "tokenizer",
        "repo_id": "microsoft/layoutlmv3-base",
        "description": "LayoutLMv3 WordPiece tokenizer",
    },
    # Table detection (DETR-style -- requires optimum export)
    {
        "name": "table_detection",
        "output": "table_detection.onnx",
        "source": "microsoft/table-transformer-detection",
        "method": "optimum_export",
        "repo_id": "microsoft/table-transformer-detection",
        "task": "object-detection",
        "opset": 17,
        "description": "Table Transformer detection model (DETR-based)",
    },
    # Table structure recognition (requires optimum export)
    {
        "name": "table_structure",
        "output": "table_structure.onnx",
        "source": "microsoft/table-transformer-structure-recognition",
        "method": "optimum_export",
        "repo_id": "microsoft/table-transformer-structure-recognition",
        "task": "object-detection",
        "opset": 17,
        "description": "Table Transformer structure recognition model",
    },
    # Sentence embeddings (pre-exported ONNX available)
    {
        "name": "embedding_model",
        "output": "embedding_model.onnx",
        "source": "sentence-transformers/all-MiniLM-L6-v2",
        "method": "huggingface_file",
        "repo_id": "sentence-transformers/all-MiniLM-L6-v2",
        "filename": "onnx/model.onnx",
        "description": "all-MiniLM-L6-v2 sentence embeddings (768-dim)",
    },
    # Embedding tokenizer
    {
        "name": "embedding_tokenizer",
        "output": "tokenizer.json",
        "source": "sentence-transformers/all-MiniLM-L6-v2",
        "method": "tokenizer",
        "repo_id": "sentence-transformers/all-MiniLM-L6-v2",
        "description": "MiniLM tokenizer",
    },
]


# ---------------------------------------------------------------------------
# Language-specific recognition models (downloaded via --languages flag)
# Detection model (ch_PP-OCRv4_det) is shared across all languages.
# ---------------------------------------------------------------------------

LANGUAGE_MODELS = {
    "en": {
        "rec_model": "rec/en_PP-OCRv4_rec/model.onnx",
        "rec_dict": "rec/en_PP-OCRv4_rec/dict.txt",
        "output_model": "ocr_rec_en.onnx",
        "output_dict": "ocr_dicts/en_dict.txt",
        "description": "PP-OCRv4 English recognition",
    },
    "ch": {
        "rec_model": "rec/ch_PP-OCRv4_rec/model.onnx",
        "rec_dict": "rec/ch_PP-OCRv4_rec/dict.txt",
        "output_model": "ocr_rec_ch.onnx",
        "output_dict": "ocr_dicts/ch_dict.txt",
        "description": "PP-OCRv4 Chinese recognition",
    },
    "ja": {
        "rec_model": "rec/japan_PP-OCRv3_rec/model.onnx",
        "rec_dict": "rec/japan_PP-OCRv3_rec/dict.txt",
        "output_model": "ocr_rec_ja.onnx",
        "output_dict": "ocr_dicts/ja_dict.txt",
        "description": "PP-OCRv3 Japanese recognition",
    },
    "ko": {
        "rec_model": "rec/korean_PP-OCRv3_rec/model.onnx",
        "rec_dict": "rec/korean_PP-OCRv3_rec/dict.txt",
        "output_model": "ocr_rec_ko.onnx",
        "output_dict": "ocr_dicts/ko_dict.txt",
        "description": "PP-OCRv3 Korean recognition",
    },
    "ar": {
        "rec_model": "rec/arabic_PP-OCRv3_rec/model.onnx",
        "rec_dict": "rec/arabic_PP-OCRv3_rec/dict.txt",
        "output_model": "ocr_rec_ar.onnx",
        "output_dict": "ocr_dicts/ar_dict.txt",
        "description": "PP-OCRv3 Arabic recognition",
    },
    "latin": {
        "rec_model": "rec/latin_PP-OCRv3_rec/model.onnx",
        "rec_dict": "rec/latin_PP-OCRv3_rec/dict.txt",
        "output_model": "ocr_rec_latin.onnx",
        "output_dict": "ocr_dicts/latin_dict.txt",
        "description": "PP-OCRv3 Latin recognition (French/German/Spanish)",
    },
}


def sha256_file(path: Path) -> str:
    """Compute SHA256 hash of a file."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def file_size_mb(path: Path) -> float:
    """Get file size in megabytes."""
    return path.stat().st_size / (1024 * 1024)


def download_huggingface_file(model: dict, output_path: Path) -> None:
    """Download a single file from HuggingFace Hub."""
    from huggingface_hub import hf_hub_download

    print(f"  Downloading {model['filename']} from {model['repo_id']}...")
    downloaded = hf_hub_download(
        repo_id=model["repo_id"],
        filename=model["filename"],
    )
    shutil.copy2(downloaded, output_path)
    print(f"  Saved to {output_path} ({file_size_mb(output_path):.1f} MB)")


def export_with_optimum(model: dict, output_path: Path) -> None:
    """Export a model to ONNX using the optimum-cli export command.

    This uses the CLI rather than Python API because the CLI supports all
    model types (including object-detection) while the Python API only
    exposes a subset of model classes.
    """
    import subprocess

    task = model.get("task", "token-classification")
    opset = model.get("opset", 17)

    print(f"  Exporting {model['repo_id']} to ONNX (task={task}, opset={opset})...")

    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)

        cmd = [
            sys.executable, "-m", "optimum.exporters.onnx",
            "--model", model["repo_id"],
            "--task", task,
            "--opset", str(opset),
            str(tmp_path),
        ]

        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            # Print stderr for debugging
            err = result.stderr.strip()
            if err:
                print(f"  stderr: {err[-500:]}")
            raise RuntimeError(f"optimum export failed (exit {result.returncode})")

        # Find the exported ONNX file
        onnx_files = list(tmp_path.glob("*.onnx"))
        if not onnx_files:
            onnx_files = list(tmp_path.glob("**/*.onnx"))
        if not onnx_files:
            raise FileNotFoundError(f"No ONNX file found after export in {tmp_path}")

        # Use the first (usually model.onnx)
        shutil.copy2(onnx_files[0], output_path)
        print(f"  Exported to {output_path} ({file_size_mb(output_path):.1f} MB)")


def save_tokenizer(model: dict, output_path: Path) -> None:
    """Download and save a HuggingFace tokenizer as JSON."""
    from transformers import AutoTokenizer

    print(f"  Downloading tokenizer from {model['repo_id']}...")
    tokenizer = AutoTokenizer.from_pretrained(model["repo_id"])

    # Save full tokenizer then extract the tokenizer.json
    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)
        tokenizer.save_pretrained(tmp_path)

        # The fast tokenizer saves tokenizer.json
        tokenizer_json = tmp_path / "tokenizer.json"
        if tokenizer_json.exists():
            shutil.copy2(tokenizer_json, output_path)
        else:
            # Fallback: save the whole tokenizer config
            tokenizer.save_pretrained(output_path.parent)
            # Rename if needed
            saved = output_path.parent / "tokenizer.json"
            if saved.exists() and saved != output_path:
                shutil.move(str(saved), str(output_path))

    print(f"  Saved tokenizer to {output_path}")


def validate_model_ids(models: list) -> bool:
    """Validate that all HuggingFace model IDs are accessible (dry run)."""
    from huggingface_hub import model_info

    all_valid = True
    for m in models:
        try:
            info = model_info(m["repo_id"])
            print(f"  [OK] {m['repo_id']} ({info.modelId})")
        except Exception as e:
            print(f"  [FAIL] {m['repo_id']}: {e}")
            all_valid = False
    return all_valid


def verify_manifest(models_dir: Path) -> bool:
    """Verify downloaded models against manifest checksums."""
    manifest_path = models_dir / "manifest.json"
    if not manifest_path.exists():
        print("No manifest.json found. Run download first.")
        return False

    with open(manifest_path) as f:
        manifest = json.load(f)

    all_ok = True
    for name, info in manifest.get("models", {}).items():
        filepath = models_dir / name
        if not filepath.exists():
            print(f"  [MISSING] {name}")
            all_ok = False
            continue

        actual_hash = sha256_file(filepath)
        expected_hash = info.get("sha256", "")
        if actual_hash == expected_hash:
            print(f"  [OK] {name}")
        else:
            print(f"  [MISMATCH] {name} (expected {expected_hash[:12]}..., got {actual_hash[:12]}...)")
            all_ok = False

    return all_ok


def download_model(model: dict, models_dir: Path, force: bool) -> dict | None:
    """Download a single model. Returns manifest entry or None if skipped."""
    output_path = models_dir / model["output"]

    # Ensure parent directory exists (for nested outputs like ocr_dicts/en_dict.txt)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if output_path.exists() and not force:
        print(f"  [SKIP] {model['output']} already exists (use --force to re-download)")
        return {
            "source": model["source"],
            "sha256": sha256_file(output_path),
            "size_bytes": output_path.stat().st_size,
        }

    method = model["method"]
    if method == "huggingface_file":
        download_huggingface_file(model, output_path)
    elif method == "optimum_export":
        export_with_optimum(model, output_path)
    elif method == "tokenizer":
        save_tokenizer(model, output_path)
    else:
        raise ValueError(f"Unknown method: {method}")

    return {
        "source": model["source"],
        "sha256": sha256_file(output_path),
        "size_bytes": output_path.stat().st_size,
    }


def main():
    parser = argparse.ArgumentParser(
        description="Download ML models for Fen document intelligence pipeline"
    )
    parser.add_argument(
        "--models-dir",
        type=Path,
        default=Path("./models"),
        help="Directory to save models (default: ./models)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Re-download models even if they exist",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate model IDs without downloading",
    )
    parser.add_argument(
        "--verify",
        action="store_true",
        help="Verify existing models against manifest checksums",
    )
    parser.add_argument(
        "--languages",
        type=str,
        default="",
        help="Comma-separated language codes for additional recognition models (en,ch,ja,ko,ar,latin)",
    )
    args = parser.parse_args()

    print("=" * 60)
    print("Fen ML Model Downloader")
    print("=" * 60)

    if args.dry_run:
        print("\n[DRY RUN] Validating HuggingFace model IDs...\n")
        # Deduplicate repo IDs
        seen = set()
        unique_models = []
        for m in MODELS:
            if m["repo_id"] not in seen:
                seen.add(m["repo_id"])
                unique_models.append(m)
        valid = validate_model_ids(unique_models)
        sys.exit(0 if valid else 1)

    if args.verify:
        print(f"\n[VERIFY] Checking models in {args.models_dir}...\n")
        valid = verify_manifest(args.models_dir)
        sys.exit(0 if valid else 1)

    # Create output directory
    args.models_dir.mkdir(parents=True, exist_ok=True)

    manifest = {"version": 1, "downloaded_at": None, "models": {}}
    downloaded = 0
    skipped = 0
    failed = 0

    for model in MODELS:
        print(f"\n[{model['name']}] {model['description']}")
        try:
            entry = download_model(model, args.models_dir, args.force)
            if entry:
                manifest["models"][model["output"]] = entry
                if args.force or not (args.models_dir / model["output"]).exists():
                    downloaded += 1
                else:
                    skipped += 1
            # If we got here without exception but the file existed, count as skip
            if not args.force and (args.models_dir / model["output"]).exists():
                skipped += 1
                downloaded = max(0, downloaded - 1)  # Correct the count
        except Exception as e:
            print(f"  [ERROR] Failed: {e}")
            failed += 1

    # Download language-specific recognition models if requested
    if args.languages:
        lang_codes = [l.strip() for l in args.languages.split(",") if l.strip()]
        for code in lang_codes:
            if code not in LANGUAGE_MODELS:
                print(f"\n[WARNING] Unknown language code: {code} (available: {', '.join(LANGUAGE_MODELS.keys())})")
                continue

            lang = LANGUAGE_MODELS[code]
            # Download recognition model
            rec_model = {
                "name": f"ocr_rec_{code}",
                "output": lang["output_model"],
                "source": "deepghs/paddleocr",
                "method": "huggingface_file",
                "repo_id": "deepghs/paddleocr",
                "filename": lang["rec_model"],
                "description": f"{lang['description']} model",
            }
            print(f"\n[ocr_rec_{code}] {lang['description']} model")
            try:
                entry = download_model(rec_model, args.models_dir, args.force)
                if entry:
                    manifest["models"][lang["output_model"]] = entry
                    downloaded += 1
            except Exception as e:
                print(f"  [ERROR] Failed: {e}")
                failed += 1

            # Download dictionary
            rec_dict = {
                "name": f"ocr_dict_{code}",
                "output": lang["output_dict"],
                "source": "deepghs/paddleocr",
                "method": "huggingface_file",
                "repo_id": "deepghs/paddleocr",
                "filename": lang["rec_dict"],
                "description": f"{lang['description']} dictionary",
            }
            print(f"\n[ocr_dict_{code}] {lang['description']} dictionary")
            try:
                entry = download_model(rec_dict, args.models_dir, args.force)
                if entry:
                    manifest["models"][lang["output_dict"]] = entry
                    downloaded += 1
            except Exception as e:
                print(f"  [ERROR] Failed: {e}")
                failed += 1

    # Write manifest
    manifest["downloaded_at"] = datetime.now(timezone.utc).isoformat()
    manifest_path = args.models_dir / "manifest.json"
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)

    # Summary
    all_outputs = [m["output"] for m in MODELS]
    all_outputs.extend(manifest["models"].keys())
    total_size = sum(
        (args.models_dir / o).stat().st_size
        for o in set(all_outputs)
        if (args.models_dir / o).exists()
    )

    print("\n" + "=" * 60)
    print("Summary")
    print("=" * 60)
    print(f"  Downloaded: {downloaded}")
    print(f"  Skipped:    {skipped}")
    print(f"  Failed:     {failed}")
    print(f"  Total size: {total_size / (1024 * 1024):.1f} MB")
    print(f"  Manifest:   {manifest_path}")
    print()

    if failed > 0:
        print("Some models failed to download. Re-run to retry.")
        sys.exit(1)


if __name__ == "__main__":
    main()
