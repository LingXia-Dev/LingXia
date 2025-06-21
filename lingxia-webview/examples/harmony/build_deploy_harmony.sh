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
LOG_TAG="LingXia"
SCREENSHOT_DEVICE_PATH="/data/local/tmp/lingxia_screenshot.jpeg"
SCREENSHOT_LOCAL_PATH="./lingxia_screenshot.jpeg"

echo -e "${BLUE}🚀 LingXia MiniApp Harmony Build & Deploy Script${NC}"
echo "=================================================="

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

# Function to build HAR library
build_har() {
    print_step "1" "Building HAR Library"

    cd ../../harmony
    echo "Building lingxia HAR library..."

    if hvigorw assembleHar; then
        echo -e "${GREEN}✅ HAR library built successfully${NC}"
    else
        echo -e "${RED}❌ Failed to build HAR library${NC}"
        exit 1
    fi

    cd ../examples/harmony
}

# Function to build HAP application
build_hap() {
    print_step "2" "Building HAP Application"

    echo "Building example HAP application..."

    if hvigorw assembleHap; then
        echo -e "${GREEN}✅ HAP application built successfully${NC}"

        if [ -f "$HAP_PATH" ]; then
            echo -e "${GREEN}✅ HAP file found: $HAP_PATH${NC}"
        else
            echo -e "${RED}❌ HAP file not found at expected path: $HAP_PATH${NC}"
            exit 1
        fi
    else
        echo -e "${RED}❌ Failed to build HAP application${NC}"
        exit 1
    fi
}

# Function to uninstall existing app
uninstall_app() {
    print_step "3" "Uninstalling Existing App"

    echo "Checking if app is already installed..."

    if hdc shell bm dump -n $APP_PACKAGE &> /dev/null; then
        echo "App is installed, uninstalling..."
        if hdc uninstall $APP_PACKAGE; then
            echo -e "${GREEN}✅ App uninstalled successfully${NC}"
        else
            echo -e "${YELLOW}⚠️  Failed to uninstall app (may not be installed)${NC}"
        fi
    else
        echo -e "${GREEN}✅ App not previously installed${NC}"
    fi
}

# Function to install HAP
install_hap() {
    print_step "4" "Installing HAP Application"

    echo "Installing HAP to device..."

    if hdc install "$HAP_PATH"; then
        echo -e "${GREEN}✅ HAP installed successfully${NC}"
    else
        echo -e "${RED}❌ Failed to install HAP${NC}"
        exit 1
    fi
}

# Function to start app
start_app() {
    print_step "5" "Starting Application"

    echo "Starting LingXia MiniApp example..."

    if hdc shell aa start -a $APP_ABILITY -b $APP_PACKAGE; then
        echo -e "${GREEN}✅ App started successfully${NC}"
        echo "Waiting 3 seconds for app to initialize..."
        sleep 3
    else
        echo -e "${RED}❌ Failed to start app${NC}"
        exit 1
    fi
}

# Function to capture screenshot
capture_screenshot() {
    print_step "6" "Capturing Screenshot"

    echo "Taking screenshot..."

    if hdc shell snapshot_display -f $SCREENSHOT_DEVICE_PATH; then
        echo -e "${GREEN}✅ Screenshot captured on device${NC}"

        echo "Downloading screenshot to local..."
        if hdc file recv $SCREENSHOT_DEVICE_PATH $SCREENSHOT_LOCAL_PATH; then
            echo -e "${GREEN}✅ Screenshot saved to: $SCREENSHOT_LOCAL_PATH${NC}"

            # Clean up device screenshot
            hdc shell rm $SCREENSHOT_DEVICE_PATH
        else
            echo -e "${RED}❌ Failed to download screenshot${NC}"
        fi
    else
        echo -e "${RED}❌ Failed to capture screenshot${NC}"
    fi
}

# Function to capture logs
capture_logs() {
    print_step "7" "Capturing Application Logs"

    echo "Capturing logs for 10 seconds..."
    echo -e "${BLUE}📝 Log output:${NC}"
    echo "----------------------------------------"

    # Clear existing logs first
    hdc shell hilog -r

    # Show logs directly in terminal (like Android build script)
    echo "Showing logs (will auto-stop after 40 seconds)..."
    timeout 40s hdc hilog | grep -E "(LingXia|MainActivity|MiniApp|WebView)" &
}

# Main execution
main() {
    echo -e "${BLUE}Starting build and deploy process...${NC}"

    # check_hdc
    # build_har
    build_hap
    # uninstall_app
    install_hap
    capture_logs
    start_app
    # capture_screenshot

    echo -e "\n${GREEN}🎉 Build and deploy completed successfully!${NC}"
    echo -e "${GREEN}📱 LingXia MiniApp should now be running on your device${NC}"
}

# Handle script arguments
case "${1:-}" in
    "build-only")
        echo -e "${BLUE}Building only (no deploy)...${NC}"
        check_hdc
        build_har
        build_hap
        echo -e "${GREEN}✅ Build completed${NC}"
        ;;
    "deploy-only")
        echo -e "${BLUE}Deploying only (assuming already built)...${NC}"
        check_hdc
        uninstall_app
        install_hap
        start_app
        capture_logs
        ;;
    "logs-only")
        echo -e "${BLUE}Capturing logs only...${NC}"
        check_hdc
        capture_logs
        ;;
    "screenshot-only")
        echo -e "${BLUE}Taking screenshot only...${NC}"
        check_hdc
        capture_screenshot
        ;;
    *)
        main
        ;;
esac
