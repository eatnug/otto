#!/bin/bash

# Download a small, capable model for Otto
# Using Qwen2.5-0.5B-Instruct - small but good at following instructions

MODEL_DIR="src-tauri/models"
MODEL_FILE="model.gguf"
MODEL_URL="https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf"

mkdir -p "$MODEL_DIR"

echo "Downloading Qwen2.5-0.5B-Instruct (Q4_K_M quantization)..."
echo "Size: ~400MB"
echo ""

curl -L -o "$MODEL_DIR/$MODEL_FILE" "$MODEL_URL" --progress-bar

if [ $? -eq 0 ]; then
    echo ""
    echo "Download complete!"
    echo "Model saved to: $MODEL_DIR/$MODEL_FILE"
else
    echo ""
    echo "Download failed. Please try again or download manually from:"
    echo "$MODEL_URL"
fi
