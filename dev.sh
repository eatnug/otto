#!/bin/bash
# Otto Development Script with Code Signing
# This signs the debug binary so macOS remembers permissions

SIGNING_IDENTITY="Apple Development: Jacob Park (9WMH775RUJ)"
BINARY_PATH="src-tauri/target/debug/otto"

# Build first
echo "Building..."
cd src-tauri && cargo build && cd ..

# Sign the binary
if [ -f "$BINARY_PATH" ]; then
    echo "Signing binary..."
    codesign -f -s "$SIGNING_IDENTITY" "$BINARY_PATH"
    echo "Signed successfully"
fi

# Run tauri dev
npm run tauri dev
