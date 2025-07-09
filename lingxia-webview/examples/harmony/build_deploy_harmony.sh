#!/bin/bash

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
APP_PACKAGE="app.lingxia.miniapp.example"
APP_ABILITY="EntryAbility"
HAP_PATH="entry/build/default/outputs/default/entry-default-signed.hap"
SCREENSHOT_DEVICE_PATH="/data/local/tmp/lingxia_screenshot.jpeg"
SCREENSHOT_LOCAL_PATH="./lingxia_screenshot.jpeg"

export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang"
export CPATH=$OHOS_NDK_HOME/native/sysroot/usr/include/:$OHOS_NDK_HOME/native/sysroot/usr/include/aarch64-linux-ohos

echo -e "${BLUE}🚀 LingXia MiniApp Harmony Build & Deploy Script${NC}"
echo "=================================================="

# Get the absolute path of the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
# Navigate to LingXia root (3 levels up from examples/harmony)
LINGXIA_ROOT="$SCRIPT_DIR/../../.."

# Function to print step headers
print_step() {
    echo -e "\n${YELLOW}📋 Step $1: $2${NC}"
    echo "----------------------------------------"
}

# Function to check if hdc is available
check_hdc() {
    if ! command -v hdc &> /dev/null; then
        echo -e "${RED}❌ Error: hdc command not found. Please install HarmonyOS SDK and add hdc to PATH.${NC}"
        exit 1
    fi

    # Check if device is connected
    if ! hdc list targets | grep -q ".*"; then
        echo -e "${RED}❌ Error: No HarmonyOS device connected. Please connect a device or start emulator.${NC}"
        exit 1
    fi

    echo -e "${GREEN}✅ HarmonyOS device connected${NC}"
}

# Function to build Rust library
build_rust() {
    print_step "1" "Building Rust Library"

    # Navigate to the LingXia root directory where Cargo.toml is located
    cd "$LINGXIA_ROOT"

    # Build Rust library with HarmonyOS target
    echo "Building Rust library for HarmonyOS..."
     env CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang" \
         cargo build --release --target=aarch64-unknown-linux-ohos

    # Copy the SO file to the example project
    SO_SOURCE="$LINGXIA_ROOT/target/aarch64-unknown-linux-ohos/release/liblingxia.so"
    SO_DEST="$SCRIPT_DIR/entry/libs/arm64-v8a/liblingxia.so"

    mkdir -p "$SCRIPT_DIR/entry/libs/arm64-v8a"
    cp "$SO_SOURCE" "$SO_DEST"

    echo -e "${GREEN}✅ Rust library copied${NC}"
    cd "$SCRIPT_DIR"
}

# Function to build and copy MiniApp assets
build_miniapp_assets() {
    print_step "2" "Building MiniApp Assets"

    ASSETS_DIR="$SCRIPT_DIR/entry/src/main/resources/rawfile"
    mkdir -p "$ASSETS_DIR"
    rm -rf "$ASSETS_DIR"/*

    echo "Copying lingxia-view files to assets..."
    cp "$LINGXIA_ROOT/lingxia-view/404.html" "$ASSETS_DIR/"
    cp "$LINGXIA_ROOT/lingxia-view/webview-bridge.js" "$ASSETS_DIR/"

    echo "Copying host app configuration..."
    cp "$LINGXIA_ROOT/examples/demo/app.json" "$ASSETS_DIR/"

    echo "Building and copying demo MiniApp..."
    cd "$LINGXIA_ROOT/examples/demo/homelxapp"
    if [ -f "package.json" ] && [ -f "vite.config.js" ]; then
        echo "Building homelxapp with Vite..."
        npm install --silent
        npm run build

        if [ -d "dist" ]; then
            echo "Copying built MiniApp to assets..."
            mkdir -p "$ASSETS_DIR/homelxapp"
            cp -R dist/* "$ASSETS_DIR/homelxapp/"
        else
            echo "Warning: dist directory not found, copying source files..."
            cp -R . "$ASSETS_DIR/homelxapp/"
        fi
    else
        echo "No Vite config found, copying source files..."
        mkdir -p "$ASSETS_DIR/homelxapp"
        cp -R "$LINGXIA_ROOT/examples/demo/homelxapp/"* "$ASSETS_DIR/homelxapp/"
    fi

    echo -e "${GREEN}✅ MiniApp assets copied${NC}"
    cd "$SCRIPT_DIR"
}

# Function to build HAR library
build_har() {
    print_step "3" "Building HAR Library"

    cd "$LINGXIA_ROOT/lingxia-webview/harmony"
    echo "Building HAR library..."
    hvigorw assembleHar
    echo -e "${GREEN}✅ HAR library built${NC}"
    cd "$SCRIPT_DIR"
}

# Function to build HAP application
build_hap() {
    print_step "4" "Building HAP Application"

    echo "Building HAP application..."
    hvigorw assembleHap
    echo -e "${GREEN}✅ HAP application built${NC}"
}

# Function to uninstall existing app
uninstall_app() {
    print_step "5" "Uninstalling App"

    hdc uninstall $APP_PACKAGE > /dev/null 2>&1 || true
    echo -e "${GREEN}✅ App uninstalled${NC}"
}

# Function to install HAP
install_hap() {
    print_step "6" "Installing HAP Application"

    hdc install "$HAP_PATH" > /dev/null 2>&1
    echo -e "${GREEN}✅ HAP installed${NC}"
}

# Function to start app
start_app() {
    print_step "7" "Starting Application"

    hdc shell aa start -a $APP_ABILITY -b $APP_PACKAGE > /dev/null 2>&1
    echo -e "${GREEN}✅ Application started${NC}"
    sleep 2
}

# Function to capture logs
capture_logs() {
    print_step "8" "Capturing Application Logs"

    echo "Clearing existing logs..."
    hdc hilog -r

    echo "Capturing logs (press Ctrl+C to stop)..."
    echo -e "${BLUE}📝 Log output:${NC}"
    echo "----------------------------------------"

    # Show logs and filter for LingXia
    timeout 30s hdc hilog | grep -E "(LingXia|MiniApp|WebView)"
}

# Main execution
main() {
    echo -e "${BLUE}Starting build and deploy process...${NC}"

    check_hdc
    build_rust
    build_miniapp_assets
    build_har
    build_hap
    uninstall_app
    install_hap
    start_app
    capture_logs

    echo -e "\n${GREEN}🎉 Build and deploy completed successfully!${NC}"
    echo -e "${GREEN}📱 LingXia MiniApp should now be running on your device${NC}"
}

# Parse arguments
SKIP_RUST=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-rust|-s)
            SKIP_RUST=true
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo -e "${YELLOW}Usage: $0 [--skip-rust|-s]${NC}"
            exit 1
            ;;
    esac
done

# Main build and deploy flow
if [ "$SKIP_RUST" = true ]; then
    echo -e "${BLUE}Starting quick build and deploy (skipping Rust)...${NC}"
else
    echo -e "${BLUE}Starting full build and deploy (including Rust)...${NC}"
fi

check_hdc

# Build phase
if [ "$SKIP_RUST" = false ]; then
    build_rust
else
    echo -e "${YELLOW}⏭️  Skipping Rust compilation${NC}"
fi

build_miniapp_assets
build_hap

# Deploy phase
uninstall_app
install_hap

# Start log capture BEFORE launching the app to catch startup logs
echo -e "${BLUE}📝 Starting log capture before app launch...${NC}"
echo "Clearing existing logs..."
timeout 2s hdc shell hilog -r >/dev/null 2>&1 || true

echo "Starting log capture in background..."
timeout 30s hdc hilog | grep -E "(LingXia|MiniApp|WebView)" &
LOG_PID=$!

# Now start the app
start_app

echo -e "${GREEN}✅ Build and deploy completed${NC}"
echo -e "${BLUE}📱 App launched, logs are being captured for 30 seconds...${NC}"
echo -e "${YELLOW}💡 Press Ctrl+C to stop log capture early, or use 'hdc hilog | grep LingXia' for manual monitoring${NC}"
 hdc fport tcp:9222 localabstract:webview_devtools_remote_$( hdc shell cat /proc/net/unix |grep devtools | grep -o '[0-9]*$')
# Wait for log capture to complete
wait $LOG_PID 2>/dev/null
echo -e "${BLUE}📝 Log capture completed${NC}"
