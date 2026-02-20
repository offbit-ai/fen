#!/usr/bin/env bash
# =============================================================================
# Fen ML Model Setup
# =============================================================================
# Creates/reuses a Python venv and runs the model download script.
# This is the single entry point for model management.
#
# Usage:
#   ./scripts/setup-models.sh                    # Download all models
#   ./scripts/setup-models.sh --force            # Re-download all models
#   ./scripts/setup-models.sh --dry-run          # Validate model IDs only
#   ./scripts/setup-models.sh --verify           # Check SHA256 hashes
#   ./scripts/setup-models.sh --models-dir /tmp  # Custom output directory
#
# Prerequisites:
#   Python 3.9+ must be installed

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VENV_DIR="$PROJECT_ROOT/.venv-models"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_ok() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_err() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check Python is available
if ! command -v python3 &>/dev/null; then
    log_err "Python 3 is required but not found. Install Python 3.9+ and try again."
    exit 1
fi

PYTHON_VERSION=$(python3 -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
log_info "Using Python $PYTHON_VERSION"

# Create venv if it doesn't exist
if [ ! -d "$VENV_DIR" ]; then
    log_info "Creating Python virtual environment at $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
    log_info "Installing dependencies (this may take a few minutes on first run)..."
    "$VENV_DIR/bin/pip" install -q --upgrade pip
    "$VENV_DIR/bin/pip" install -q \
        "optimum[onnxruntime]>=1.17" \
        "transformers>=4.40" \
        "huggingface_hub>=0.20" \
        "onnx>=1.15" \
        "timm"
    log_ok "Virtual environment ready"
else
    log_info "Using existing virtual environment at $VENV_DIR"
fi

# Run the download script, forwarding all arguments
"$VENV_DIR/bin/python" "$SCRIPT_DIR/download-models.py" "$@"
