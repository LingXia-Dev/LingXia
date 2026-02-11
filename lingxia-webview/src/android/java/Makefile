# Makefile for LingXia WebView Android Java compilation
# Compiles Java files and packages them into JAR format for Android projects

# Environment variables (must be set by caller)
# TARGET_DIR - Build output directory (set by build.rs)
# ANDROID_SDK_ROOT - Android SDK path (must be set by user)

# Validate required environment variables
ifndef ANDROID_SDK_ROOT
$(error ANDROID_SDK_ROOT environment variable is not set. Please set it to your PATH)
endif

ifndef TARGET_DIR
$(error TARGET_DIR environment variable is not set. This should be set by build.rs)
endif

# Auto-detect latest Android API level
ANDROID_PLATFORMS_DIR = $(ANDROID_SDK_ROOT)/platforms
LATEST_ANDROID_API = $(shell ls $(ANDROID_PLATFORMS_DIR) | grep '^android-[0-9]*$$' | sed 's/android-//' | sort -n | tail -1)
SELECTED_ANDROID_API = $(if $(ANDROID_API_LEVEL),$(ANDROID_API_LEVEL),$(LATEST_ANDROID_API))
ANDROID_JAR = $(ANDROID_PLATFORMS_DIR)/android-$(SELECTED_ANDROID_API)/android.jar

# Build paths
JAR_OUTPUT = $(TARGET_DIR)/lingxia-webview.jar
JAVA_FILES = $(wildcard *.java)
CLASS_FILES = $(JAVA_FILES:%.java=$(TARGET_DIR)/com/lingxia/webview/%.class)


# Default target
all: check-env $(JAR_OUTPUT)

# Check environment and detect Android API
check-env:
	@echo "Checking Android environment..."
	@if [ ! -d "$(ANDROID_PLATFORMS_DIR)" ]; then \
		echo "Error: Android platforms directory not found at $(ANDROID_PLATFORMS_DIR)"; \
		echo "Please ensure ANDROID_SDK_ROOT is set correctly: $(ANDROID_SDK_ROOT)"; \
		exit 1; \
	fi
	@if [ -z "$(LATEST_ANDROID_API)" ]; then \
		echo "Error: No Android API levels found in $(ANDROID_PLATFORMS_DIR)"; \
		echo "Please install Android SDK platforms"; \
		exit 1; \
	fi
	@if [ ! -f "$(ANDROID_JAR)" ]; then \
		echo "Error: Android JAR not found at $(ANDROID_JAR)"; \
		echo "Selected API level: $(SELECTED_ANDROID_API) (latest: $(LATEST_ANDROID_API))"; \
		exit 1; \
	fi
	@echo "Using Android API $(SELECTED_ANDROID_API): $(ANDROID_JAR)"

# Create target directory
$(TARGET_DIR):
	mkdir -p $(TARGET_DIR)

# Compile all Java files at once (to handle dependencies)
# Note: -d creates package directory structure automatically
$(CLASS_FILES): $(JAVA_FILES) | $(TARGET_DIR)
	@echo "Compiling Java files with API $(SELECTED_ANDROID_API)..."
	javac -source 8 -target 8 -d $(TARGET_DIR) -cp $(ANDROID_JAR) $(JAVA_FILES)
	@echo "Compilation completed"

# Create JAR file from compiled classes and clean up temporary class files
$(JAR_OUTPUT): $(CLASS_FILES)
	@echo "Creating JAR file..."
	jar cf $(JAR_OUTPUT) -C $(TARGET_DIR) .
	@echo "JAR created: $(JAR_OUTPUT)"
	@echo "Cleaning up temporary class files..."
	rm -rf $(TARGET_DIR)/com
	@echo "Build completed successfully"

# Clean build artifacts
clean:
	rm -rf $(TARGET_DIR)

# Show variables (for debugging)
debug:
	@echo "ANDROID_JAR: $(ANDROID_JAR)"
	@echo "TARGET_DIR: $(TARGET_DIR)"
	@echo "JAVA_FILES: $(JAVA_FILES)"
	@echo "CLASS_FILES: $(CLASS_FILES)"
	@echo "JAR_OUTPUT: $(JAR_OUTPUT)"

.PHONY: all clean debug check-env
