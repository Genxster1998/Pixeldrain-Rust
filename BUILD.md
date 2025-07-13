# 🚀 PixelDrain Build System

This document describes how to build and package PixelDrain for different platforms.

## 📋 Prerequisites

### All Platforms
- Rust (latest stable)
- Cargo
- Git

### macOS
- Homebrew (for installing dependencies)
- ImageMagick (for icon conversion)
- create-dmg (for DMG creation)

### Windows
- PowerShell
- ImageMagick (for icon conversion)
- NSIS (for installer creation)
- Chocolatey (for installing dependencies)

### Linux
- ImageMagick (for icon conversion)
- dpkg-dev (for DEB packages)
- appimagetool (for AppImage creation)

## 🛠️ Quick Start

### Using Make (Recommended)

```bash
# Show build information
make info

# Build the application
make build

# Package for current platform
make package

# Install packaging dependencies
make install-deps

# Full release workflow
make release

# Show all available commands
make help
```

### Using Scripts Directly

#### macOS/Linux
```bash
# Make script executable
chmod +x scripts/build.sh

# Run build script
./scripts/build.sh
```

#### Windows
```powershell
# Run PowerShell script
.\scripts\build.ps1

# Or with parameters
.\scripts\build.ps1 -SkipBuild -Version "0.1.0"
```

## 📦 Package Types

### macOS
- **App Bundle** (`.app`) - Native macOS application
- **DMG** (`.dmg`) - Disk image for distribution

### Windows
- **Installer** (`.exe`) - NSIS-based installer
- **Portable** (`.zip`) - Portable version

### Linux
- **DEB Package** (`.deb`) - Debian/Ubuntu package
- **AppImage** (`.AppImage`) - Universal Linux package

## 🔧 Build Configuration

### Icons
The build system automatically handles icon conversion:
- Source: `assets/icon.png` (256x256 or larger)
- Windows: Converts to `assets/icon.ico`
- macOS: Uses PNG directly
- Linux: Uses PNG directly

### Version Information
Update version in:
- `Cargo.toml` - Main version
- `scripts/build.sh` - Build script version
- `scripts/build.ps1` - PowerShell script version

## 🏗️ Build Process

### 1. Application Build
```bash
cargo build --release
```

### 2. Icon Processing
- Converts PNG to ICO for Windows
- Embeds icons in executables
- Creates platform-specific icon files

### 3. Package Creation
- **macOS**: Creates `.app` bundle and `.dmg`
- **Windows**: Creates installer and portable package
- **Linux**: Creates DEB package and AppImage

### 4. Output
All packages are created in the `dist/` directory.

## 🔍 Troubleshooting

### Common Issues

#### ImageMagick Not Found
```bash
# macOS
brew install imagemagick

# Ubuntu/Debian
sudo apt-get install imagemagick

# Windows
choco install imagemagick
```

#### create-dmg Not Found (macOS)
```bash
brew install create-dmg
```

#### NSIS Not Found (Windows)
```powershell
choco install nsis
```

#### appimagetool Not Found (Linux)
```bash
wget -O appimagetool https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x appimagetool
sudo mv appimagetool /usr/local/bin/
```

### Build Errors

#### Windows Icon Embedding
If you get errors about missing `.ico` file:
```bash
make convert-icons
```

#### Permission Errors
```bash
# Make scripts executable
chmod +x scripts/*.sh
```

#### Clean Build
```bash
make clean
make build
```

## 🎯 Advanced Usage

### Cross-Platform Building

For cross-platform builds, you'll need to set up cross-compilation:

```bash
# Install cross-compilation targets
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin

# Build for specific target
cargo build --release --target x86_64-pc-windows-msvc
```

### Custom Build Scripts

You can create custom build scripts by modifying the existing ones:

```bash
# Copy and modify
cp scripts/build.sh scripts/build-custom.sh
# Edit scripts/build-custom.sh
```

### CI/CD Integration

The build scripts are designed to work in CI/CD environments:

```yaml
# GitHub Actions example
- name: Build and Package
  run: |
    make install-deps
    make package
```

## 📁 Directory Structure

```
PixelDrain/
├── assets/
│   └── icon.png              # Source icon
├── build/
│   ├── pixeldrain.desktop    # Linux desktop entry
│   └── windows.rc            # Windows resource file
├── scripts/
│   ├── build.sh              # Unix build script
│   └── build.ps1             # Windows build script
├── dist/                     # Output directory
├── target/                   # Rust build output
├── build.rs                  # Rust build script
├── Makefile                  # Build automation
└── BUILD.md                  # This file
```

## 🔄 Development Workflow

### Daily Development
```bash
make run      # Run in development mode
make check    # Check code quality
make test     # Run tests
make fmt      # Format code
```

### Release Preparation
```bash
make clean    # Clean previous builds
make install-deps  # Install packaging tools
make convert-icons # Convert icons
make package  # Create packages
```

### Quality Assurance
```bash
make dev      # Full development workflow
make release  # Full release workflow
```

## 📝 Notes

- The build system automatically detects your platform
- All packages include proper metadata and icons
- Windows builds embed version information in the executable
- Linux packages include desktop integration
- macOS builds create proper app bundles with DMG distribution

## 🤝 Contributing

When adding new build targets or modifying the build system:

1. Update this documentation
2. Test on all target platforms
3. Ensure backward compatibility
4. Add appropriate error handling

## 📄 License

This build system is part of the PixelDrain project and is licensed under the MIT License. 