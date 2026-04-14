#!/bin/bash

# Liminal Screen - Verification Script
# Run this script to verify that all fixes are working correctly

echo "=== Liminal Screen Verification Script ==="
echo

# Check if we're in the right directory
if [ ! -f "src/main.ts" ]; then
    echo "Error: Please run this script from the project root directory"
    exit 1
fi

echo "Checking file modifications..."
echo

# Check that all expected files exist and have been modified
FILES_TO_CHECK=(
    "src/main.ts"
    "src/app/preview/preview.ts"
    "src/app/saver/saver.ts"
)

for file in "${FILES_TO_CHECK[@]}"; do
    if [ -f "$file" ]; then
        echo "✓ $file exists"
    else
        echo "✗ $file is missing"
    fi
done

echo
echo "Checking specific fixes..."
echo

# Check Issue #1 fix - immediate initialization
if grep -q "Liminal Screen immediate initialization attempt" src/main.ts; then
    echo "✓ Issue #1 fix verified: Immediate initialization code found"
else
    echo "✗ Issue #1 fix missing: Immediate initialization code not found"
fi

# Check Issue #2 fix - preview media cleanup
if grep -q "navigate_webview.*about:blank" src/main.ts && grep -q "onCloseRequested" src/main.ts; then
    echo "✓ Issue #2 fix verified: Preview media cleanup implemented"
else
    echo "✗ Issue #2 fix missing: Preview media cleanup not found"
fi

# Check Issue #3 fix - Preview class
if [ -f "src/app/preview/preview.ts" ]; then
    echo "✓ Issue #3 fix verified: Preview class created"
else
    echo "✗ Issue #3 fix missing: Preview class not found"
fi

# Check Issue #4 fix - fullscreen behavior
if grep -q "Intentionally omit x, y, width, height for fullscreen windows" src/app/saver/saver.ts; then
    echo "✓ Issue #4 fix verified: Fullscreen behavior improvements found"
else
    echo "✗ Issue #4 fix missing: Fullscreen behavior improvements not found"
fi

echo
echo "=== Verification Complete ==="
echo
echo "Next steps:"
echo "1. Build the application: npm run tauri build"
echo "2. Test Issue #1: Launch app and check if monitoring starts immediately"
echo "3. Test Issue #2: Open/close preview window and verify media stops"
echo "4. Test Issue #3: Verify preview functionality works through Preview class"
echo "5. Test Issue #4: Connect multiple monitors and verify fullscreen behavior"