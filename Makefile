# PixelDrain Build Makefile
# Usage: make [target]

# Configuration
APP_NAME = pixeldrain
VERSION = 0.1.0
AUTHOR = Genxster1998

# Directories
BUILD_DIR = target
DIST_DIR = dist
ASSETS_DIR = assets
SCRIPTS_DIR = scripts

# Detect OS
ifeq ($(OS),Windows_NT)
    DETECTED_OS = windows
    BUILD_SCRIPT = $(SCRIPTS_DIR)/build.ps1
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Darwin)
        DETECTED_OS = macos
        BUILD_SCRIPT = $(SCRIPTS_DIR)/build.sh
    else
        DETECTED_OS = linux
        BUILD_SCRIPT = $(SCRIPTS_DIR)/build.sh
    endif
endif

# Default target
.PHONY: all
all: build

# Build the application
.PHONY: build
build:
	@echo "🔨 Building $(APP_NAME) for $(DETECTED_OS)..."
	cargo build --release

# Clean build artifacts
.PHONY: clean
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf $(DIST_DIR)

# Run the application
.PHONY: run
run:
	@echo "🚀 Running $(APP_NAME)..."
	cargo run

# Check code
.PHONY: check
check:
	@echo "🔍 Checking code..."
	cargo check
	cargo clippy

# Test the application
.PHONY: test
test:
	@echo "🧪 Running tests..."
	cargo test

# Format code
.PHONY: fmt
fmt:
	@echo "🎨 Formatting code..."
	cargo fmt

# Package for current platform
.PHONY: package
package: build
	@echo "📦 Packaging for $(DETECTED_OS)..."
ifeq ($(DETECTED_OS),windows)
	powershell -ExecutionPolicy Bypass -File $(BUILD_SCRIPT)
else
	chmod +x $(BUILD_SCRIPT)
	$(BUILD_SCRIPT)
endif

# Package for all platforms (requires cross-compilation setup)
.PHONY: package-all
package-all:
	@echo "🌍 Packaging for all platforms..."
	@echo "This requires cross-compilation setup. See scripts/ for individual platform scripts."

# Install dependencies for packaging
.PHONY: install-deps
install-deps:
	@echo "📥 Installing packaging dependencies..."
ifeq ($(DETECTED_OS),macos)
	@if ! command -v create-dmg >/dev/null 2>&1; then \
		echo "Installing create-dmg..."; \
		brew install create-dmg; \
	fi
	@if ! command -v convert >/dev/null 2>&1; then \
		echo "Installing ImageMagick..."; \
		brew install imagemagick; \
	fi
else ifeq ($(DETECTED_OS),linux)
	@if ! command -v dpkg-deb >/dev/null 2>&1; then \
		echo "Installing dpkg-dev..."; \
		sudo apt-get install dpkg-dev; \
	fi
	@if ! command -v convert >/dev/null 2>&1; then \
		echo "Installing ImageMagick..."; \
		sudo apt-get install imagemagick; \
	fi
	@if ! command -v appimagetool >/dev/null 2>&1; then \
		echo "Installing appimagetool..."; \
		wget -O appimagetool https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage; \
		chmod +x appimagetool; \
		sudo mv appimagetool /usr/local/bin/; \
	fi
else ifeq ($(DETECTED_OS),windows)
	@if ! command -v makensis >/dev/null 2>&1; then \
		echo "Installing NSIS..."; \
		choco install nsis; \
	fi
	@if ! command -v magick >/dev/null 2>&1; then \
		echo "Installing ImageMagick..."; \
		choco install imagemagick; \
	fi
endif

# Convert icon formats
.PHONY: convert-icons
convert-icons:
	@echo "🖼️ Converting icons..."
ifeq ($(DETECTED_OS),windows)
	@if command -v magick >/dev/null 2>&1; then \
		magick $(ASSETS_DIR)/dark-icon.png -resize 256x256 $(ASSETS_DIR)/icon.ico; \
	else \
		echo "ImageMagick not found. Please install it first."; \
	fi
else
	@if command -v convert >/dev/null 2>&1; then \
		convert $(ASSETS_DIR)/dark-icon.png -resize 256x256 $(ASSETS_DIR)/icon.ico; \
	else \
		echo "ImageMagick not found. Please install it first."; \
	fi
endif

# Show help
.PHONY: help
help:
	@echo "PixelDrain Build System"
	@echo "======================"
	@echo ""
	@echo "Available targets:"
	@echo "  build        - Build the application in release mode"
	@echo "  clean        - Clean build artifacts"
	@echo "  run          - Run the application"
	@echo "  check        - Check code with cargo check and clippy"
	@echo "  test         - Run tests"
	@echo "  fmt          - Format code"
	@echo "  package      - Build and package for current platform"
	@echo "  package-all  - Package for all platforms (requires setup)"
	@echo "  install-deps - Install packaging dependencies"
	@echo "  convert-icons- Convert PNG icon to ICO format"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Detected OS: $(DETECTED_OS)"
	@echo "Build script: $(BUILD_SCRIPT)"

# Development workflow
.PHONY: dev
dev: fmt check test build

# Release workflow
.PHONY: release
release: clean install-deps convert-icons package

# Show build info
.PHONY: info
info:
	@echo "Build Information"
	@echo "================="
	@echo "App Name: $(APP_NAME)"
	@echo "Version: $(VERSION)"
	@echo "Author: $(AUTHOR)"
	@echo "OS: $(DETECTED_OS)"
	@echo "Build Script: $(BUILD_SCRIPT)"
	@echo "Build Dir: $(BUILD_DIR)"
	@echo "Dist Dir: $(DIST_DIR)" 