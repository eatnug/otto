#!/bin/bash
# Auto-sign otto binary whenever it changes (polling version)

BINARY="src-tauri/target/debug/otto"
IDENTITY="Apple Development: Jacob Park (9WMH775RUJ)"
LAST_SIZE=""
LAST_MOD=""

echo "Watching $BINARY for changes... (Ctrl+C to stop)"

while true; do
    if [ -f "$BINARY" ]; then
        CURRENT_MOD=$(stat -f %m "$BINARY" 2>/dev/null)
        CURRENT_SIZE=$(stat -f %z "$BINARY" 2>/dev/null)
        # Only sign if modification time AND size changed (avoids re-signing after codesign)
        if [ "$CURRENT_MOD:$CURRENT_SIZE" != "$LAST_MOD:$LAST_SIZE" ] && [ -n "$CURRENT_MOD" ]; then
            LAST_MOD="$CURRENT_MOD"
            LAST_SIZE="$CURRENT_SIZE"
            sleep 0.5  # Wait for write to complete
            # Check again after sleep to make sure it's stable
            NEW_SIZE=$(stat -f %z "$BINARY" 2>/dev/null)
            if [ "$NEW_SIZE" = "$CURRENT_SIZE" ]; then
                codesign -f -s "$IDENTITY" "$BINARY" 2>/dev/null && echo "✓ Signed at $(date '+%H:%M:%S')"
                # Update after signing
                LAST_MOD=$(stat -f %m "$BINARY" 2>/dev/null)
                LAST_SIZE=$(stat -f %z "$BINARY" 2>/dev/null)
            fi
        fi
    fi
    sleep 2
done
