#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DRIVER_SRC="$REPO_ROOT/virtual-mic-driver"
BUILD_DIR="$DRIVER_SRC/build"

XCODEPROJ="$DRIVER_SRC/BlackHole.xcodeproj"
TARGET="VerbalixMicrophone"
DERIVED_DATA="$BUILD_DIR/DerivedData"
PRODUCTS_DIR="$BUILD_DIR/Products"

if [ ! -d "$XCODEPROJ" ]; then
    echo "ERROR: Xcode project not found at $XCODEPROJ" >&2
    exit 1
fi

if ! command -v xcodebuild &>/dev/null; then
    echo "ERROR: xcodebuild not found. Install Xcode and Xcode Command Line Tools." >&2
    exit 1
fi

echo "Building VerbalixMicrophone.driver..."
echo "  Source: $DRIVER_SRC"
echo "  Output: $PRODUCTS_DIR"

mkdir -p "$BUILD_DIR"
mkdir -p "$PRODUCTS_DIR"

xcodebuild \
    -project "$XCODEPROJ" \
    -target "$TARGET" \
    -configuration Release \
    BUILD_DIR="$PRODUCTS_DIR" \
    BUILD_ROOT="$PRODUCTS_DIR" \
    OBJROOT="$BUILD_DIR/obj" \
    CODE_SIGN_IDENTITY="-" \
    CODE_SIGNING_REQUIRED=NO \
    CODE_SIGNING_ALLOWED=NO \
    2>&1

DRIVER_PATH="$PRODUCTS_DIR/Release/VerbalixMicrophone.driver"

if [ ! -d "$DRIVER_PATH" ]; then
    echo "ERROR: Build succeeded but VerbalixMicrophone.driver not found at expected path:" >&2
    echo "  $DRIVER_PATH" >&2
    echo "Products directory contents:" >&2
    find "$PRODUCTS_DIR" -name "*.driver" 2>/dev/null >&2 || true
    exit 1
fi

echo ""
echo "Build successful."
echo "Driver: $DRIVER_PATH"
