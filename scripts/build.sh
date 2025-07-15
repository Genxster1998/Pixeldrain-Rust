#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="pixeldrain"
VERSION="0.1.0"
AUTHOR="Genxster1998"
DESCRIPTION="Upload and manage files with PixelDrain"
LICENSE="MIT"

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/target"
DIST_DIR="$PROJECT_DIR/dist"
ASSETS_DIR="$PROJECT_DIR/assets"

echo -e "${BLUE}🚀 PixelDrain Build Script${NC}"
echo -e "${BLUE}========================${NC}"

# Create necessary directories
mkdir -p "$DIST_DIR"
mkdir -p "$BUILD_DIR"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to convert PNG to ICO for Windows
convert_png_to_ico() {
    local png_file="$1"
    local ico_file="$2"
    
    if command_exists convert; then
        echo -e "${YELLOW}Converting PNG to ICO...${NC}"
        convert "$png_file" -resize 256x256 "$ico_file"
    elif command_exists magick; then
        echo -e "${YELLOW}Converting PNG to ICO using ImageMagick...${NC}"
        magick "$png_file" -resize 256x256 "$ico_file"
    else
        echo -e "${RED}Error: ImageMagick not found. Please install it to convert PNG to ICO.${NC}"
        echo -e "${YELLOW}You can install it with: brew install imagemagick (macOS) or apt-get install imagemagick (Ubuntu)${NC}"
        exit 1
    fi
}

# Function to build for a specific target
build_target() {
    local target="$1"
    local features="$2"
    
    echo -e "${BLUE}Building for $target...${NC}"
    
    if [ -n "$features" ]; then
        cargo build --release --target "$target" --features "$features"
    else
        cargo build --release --target "$target"
    fi
}

# Function to create macOS app bundle
create_macos_app() {
    echo -e "${BLUE}Creating macOS app bundle...${NC}"
    
    local app_dir="$DIST_DIR/PixelDrain.app"
    local contents_dir="$app_dir/Contents"
    local macos_dir="$contents_dir/MacOS"
    local resources_dir="$contents_dir/Resources"
    
    # Create app structure
    mkdir -p "$macos_dir"
    mkdir -p "$resources_dir"
    
    # Copy executable
    cp "$BUILD_DIR/release/$APP_NAME" "$macos_dir/"
    chmod +x "$macos_dir/$APP_NAME"
    
    # Copy icon
    cp "$ASSETS_DIR/dark-icon.png" "$resources_dir/"
    cp "$ASSETS_DIR/light-icon.png" "$resources_dir/"
    
    # Create Info.plist
    cat > "$contents_dir/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>dark-icon.png</string>
    <key>CFBundleIdentifier</key>
    <string>com.$AUTHOR.$APP_NAME</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>PixelDrain</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
EOF
    
    echo -e "${GREEN}✅ macOS app bundle created: $app_dir${NC}"
}

# Function to create macOS DMG
create_macos_dmg() {
    echo -e "${BLUE}Creating macOS DMG...${NC}"
    
    if ! command_exists create-dmg; then
        echo -e "${YELLOW}create-dmg not found. Installing...${NC}"
        if command_exists brew; then
            brew install create-dmg
        else
            echo -e "${RED}Error: create-dmg not found and Homebrew not available.${NC}"
            echo -e "${YELLOW}Please install create-dmg manually: https://github.com/create-dmg/create-dmg${NC}"
            return 1
        fi
    fi
    
    local dmg_name="PixelDrain-$VERSION.dmg"
    local app_dir="$DIST_DIR/PixelDrain.app"

    # Delete existing DMG if it exists to avoid hdiutil 'File exists' error
    if [ -f "$DIST_DIR/$dmg_name" ]; then
        echo -e "${YELLOW}Removing existing DMG: $DIST_DIR/$dmg_name${NC}"
        rm "$DIST_DIR/$dmg_name"
    fi
    # AdHoc Sign the app
    codesign --force --deep --sign - "$app_dir"
    
    create-dmg \
        --volname "PixelDrain $VERSION" \
        --volicon "$ASSETS_DIR/dark-icon.png" \
        --window-pos 200 120 \
        --window-size 600 300 \
        --icon-size 100 \
        --icon "PixelDrain.app" 175 120 \
        --hide-extension "PixelDrain.app" \
        --app-drop-link 425 120 \
        "$DIST_DIR/$dmg_name" \
        "$app_dir"
    
    echo -e "${GREEN}✅ macOS DMG created: $DIST_DIR/$dmg_name${NC}"
}

# Function to create Windows installer
create_windows_installer() {
    echo -e "${BLUE}Creating Windows installer...${NC}"
    
    # Convert PNG to ICO if needed
    if [ ! -f "$ASSETS_DIR/icon.ico" ]; then
        convert_png_to_ico "$ASSETS_DIR/dark-icon.png" "$ASSETS_DIR/icon.ico"
    fi
    
    # Create NSIS script
    local nsis_script="$DIST_DIR/installer.nsi"
    cat > "$nsis_script" << EOF
!include "MUI2.nsh"

; General
Name "PixelDrain"
OutFile "PixelDrain-$VERSION-Setup.exe"
InstallDir "\$PROGRAMFILES\\PixelDrain"
InstallDirRegKey HKCU "Software\\PixelDrain" ""

; Request application privileges
RequestExecutionLevel admin

; Interface Settings
!define MUI_ABORTWARNING
!define MUI_ICON "$ASSETS_DIR/icon.ico"
!define MUI_UNICON "$ASSETS_DIR/icon.ico"

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "English"

; Installer Sections
Section "Install"
    SetOutPath "\$INSTDIR"
    File "$BUILD_DIR/x86_64-pc-windows-msvc/release/$APP_NAME.exe"
    File "$ASSETS_DIR/icon.ico"
    
    ; Create uninstaller
    WriteUninstaller "\$INSTDIR\\Uninstall.exe"
    
    ; Create start menu shortcut
    CreateDirectory "\$SMPROGRAMS\\PixelDrain"
    CreateShortCut "\$SMPROGRAMS\\PixelDrain\\PixelDrain.lnk" "\$INSTDIR\\$APP_NAME.exe" "" "\$INSTDIR\\icon.ico"
    CreateShortCut "\$SMPROGRAMS\\PixelDrain\\Uninstall.lnk" "\$INSTDIR\\Uninstall.exe"
    
    ; Create desktop shortcut
    CreateShortCut "\$DESKTOP\\PixelDrain.lnk" "\$INSTDIR\\$APP_NAME.exe" "" "\$INSTDIR\\icon.ico"
    
    ; Registry information for add/remove programs
    WriteRegStr HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "DisplayName" "PixelDrain"
    WriteRegStr HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "UninstallString" "\$INSTDIR\\Uninstall.exe"
    WriteRegStr HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "DisplayIcon" "\$INSTDIR\\icon.ico"
    WriteRegStr HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "Publisher" "$AUTHOR"
    WriteRegStr HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "DisplayVersion" "$VERSION"
    WriteRegDWORD HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "NoModify" 1
    WriteRegDWORD HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain" "NoRepair" 1
SectionEnd

; Uninstaller Section
Section "Uninstall"
    ; Remove files
    Delete "\$INSTDIR\\$APP_NAME.exe"
    Delete "\$INSTDIR\\icon.ico"
    Delete "\$INSTDIR\\Uninstall.exe"
    
    ; Remove shortcuts
    Delete "\$SMPROGRAMS\\PixelDrain\\PixelDrain.lnk"
    Delete "\$SMPROGRAMS\\PixelDrain\\Uninstall.lnk"
    RMDir "\$SMPROGRAMS\\PixelDrain"
    Delete "\$DESKTOP\\PixelDrain.lnk"
    
    ; Remove registry keys
    DeleteRegKey HKLM "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PixelDrain"
    
    ; Remove install directory
    RMDir "\$INSTDIR"
SectionEnd
EOF
    
    # Build installer if makensis is available
    if command_exists makensis; then
        makensis "$nsis_script"
        echo -e "${GREEN}✅ Windows installer created: $DIST_DIR/PixelDrain-$VERSION-Setup.exe${NC}"
    else
        echo -e "${YELLOW}makensis not found. NSIS script created: $nsis_script${NC}"
        echo -e "${YELLOW}Please install NSIS and run: makensis $nsis_script${NC}"
    fi
}
#rm $DIST_DIR/installer.nsi

# Function to create Linux packages
create_linux_packages() {
    echo -e "${BLUE}Creating Linux packages...${NC}"
    
    # Create DEB package structure
    local deb_dir="$DIST_DIR/deb"
    local debian_dir="$deb_dir/DEBIAN"
    local usr_bin_dir="$deb_dir/usr/bin"
    local usr_share_applications_dir="$deb_dir/usr/share/applications"
    local usr_share_icons_dir="$deb_dir/usr/share/icons/hicolor/256x256/apps"
    
    mkdir -p "$debian_dir"
    mkdir -p "$usr_bin_dir"
    mkdir -p "$usr_share_applications_dir"
    mkdir -p "$usr_share_icons_dir"
    
    # Copy executable
    if [ ! -f "$BUILD_DIR/release/$APP_NAME" ]; then
        echo -e "${RED}Error: Executable not found at $BUILD_DIR/release/$APP_NAME${NC}"
        return 1
    fi
    cp "$BUILD_DIR/release/$APP_NAME" "$usr_bin_dir/"
    chmod +x "$usr_bin_dir/$APP_NAME"
    
    # Copy desktop file
    if [ ! -f "$PROJECT_DIR/build/pixeldrain.desktop" ]; then
        echo -e "${RED}Error: Desktop file not found at $PROJECT_DIR/build/pixeldrain.desktop${NC}"
        return 1
    fi
    cp "$PROJECT_DIR/build/pixeldrain.desktop" "$usr_share_applications_dir/"
    
    # Copy icon
    if [ ! -f "$ASSETS_DIR/dark-icon.png" ]; then
        echo -e "${RED}Error: Icon not found at $ASSETS_DIR/dark-icon.png${NC}"
        return 1
    fi
    cp "$ASSETS_DIR/dark-icon.png" "$usr_share_icons_dir/pixeldrain.png"
    
    # Create control file
    cat > "$debian_dir/control" << EOF
Package: pixeldrain
Version: $VERSION
Section: net
Priority: optional
Architecture: amd64
Depends: libc6
Maintainer: $AUTHOR
Description: $DESCRIPTION
 PixelDrain is a modern file upload and management client
 with support for anonymous and authenticated uploads,
 file sharing, and list management.
EOF
    
    # Create postinst script
    cat > "$debian_dir/postinst" << EOF
#!/bin/bash
set -e

# Update desktop database
update-desktop-database /usr/share/applications

# Update icon cache
gtk-update-icon-cache -f -t /usr/share/icons/hicolor

exit 0
EOF
    chmod +x "$debian_dir/postinst"
    
    # Create postrm script
    cat > "$debian_dir/postrm" << EOF
#!/bin/bash
set -e

# Update desktop database
update-desktop-database /usr/share/applications

# Update icon cache
gtk-update-icon-cache -f -t /usr/share/icons/hicolor

exit 0
EOF
    chmod +x "$debian_dir/postrm"
    
    # Build DEB package
    if command_exists dpkg-deb; then
        dpkg-deb --build "$deb_dir" "$DIST_DIR/pixeldrain_${VERSION}_amd64.deb"
        echo -e "${GREEN}✅ DEB package created: $DIST_DIR/pixeldrain_${VERSION}_amd64.deb${NC}"
    else
        echo -e "${YELLOW}dpkg-deb not found. DEB package structure created in: $deb_dir${NC}"
    fi
    
    # Create AppImage
    if command_exists appimagetool; then
        echo -e "${BLUE}Creating AppImage...${NC}"
        
        local appimage_dir="$DIST_DIR/AppDir"
        mkdir -p "$appimage_dir"
        
        # Check if required files exist
        if [ ! -f "$BUILD_DIR/release/$APP_NAME" ]; then
            echo -e "${RED}Error: Executable not found at $BUILD_DIR/release/$APP_NAME${NC}"
            return 1
        fi
        
        if [ ! -f "$PROJECT_DIR/build/pixeldrain.desktop" ]; then
            echo -e "${RED}Error: Desktop file not found at $PROJECT_DIR/build/pixeldrain.desktop${NC}"
            return 1
        fi
        
        if [ ! -f "$ASSETS_DIR/dark-icon.png" ]; then
            echo -e "${RED}Error: Icon not found at $ASSETS_DIR/dark-icon.png${NC}"
            return 1
        fi
        
        # Copy files to AppDir
        cp "$BUILD_DIR/release/$APP_NAME" "$appimage_dir/$APP_NAME"
        chmod +x "$appimage_dir/$APP_NAME"
        cp "$ASSETS_DIR/dark-icon.png" "$appimage_dir/pixeldrain.png"
        cp "$PROJECT_DIR/build/pixeldrain.desktop" "$appimage_dir/"
        
        # Create AppRun script
        cat > "$appimage_dir/AppRun" << EOF
#!/bin/bash
HERE="\$(dirname "\$(readlink -f "\${0}")")"
export PATH="\${HERE}:\${PATH}"
export LD_LIBRARY_PATH="\${HERE}:\${LD_LIBRARY_PATH}"
exec "\${HERE}/$APP_NAME" "\$@"
EOF
        chmod +x "$appimage_dir/AppRun"
        
        # Clean up any existing AppImage
        if [ -f "$DIST_DIR/PixelDrain-$VERSION-x86_64.AppImage" ]; then
            rm "$DIST_DIR/PixelDrain-$VERSION-x86_64.AppImage"
        fi
        
        ARCH=x86_64 appimagetool "$appimage_dir" "$DIST_DIR/PixelDrain-$VERSION-x86_64.AppImage"
        
        if [ -f "$DIST_DIR/PixelDrain-$VERSION-x86_64.AppImage" ]; then
            echo -e "${GREEN}✅ AppImage created: $DIST_DIR/PixelDrain-$VERSION-x86_64.AppImage${NC}"
        else
            echo -e "${RED}❌ AppImage creation failed${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}appimagetool not found. AppImage not created.${NC}"
        echo -e "${YELLOW}Install with: wget -O appimagetool https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage && chmod +x appimagetool${NC}"
    fi
}

# Main build process
main() {
    local target_os="$(uname -s)"
    
    echo -e "${BLUE}Detected OS: $target_os${NC}"
    
    # Build the application
    echo -e "${BLUE}Building application...${NC}"
    cargo build --release
    
    case "$target_os" in
        "Darwin")
            echo -e "${BLUE}Building for macOS...${NC}"
            create_macos_app
            create_macos_dmg
            ;;
        "Linux")
            echo -e "${BLUE}Building for Linux...${NC}"
            create_linux_packages
            ;;
        "MINGW"*|"MSYS"*|"CYGWIN"*)
            echo -e "${BLUE}Building for Windows...${NC}"
            create_windows_installer
            ;;
        *)
            echo -e "${YELLOW}Unknown OS: $target_os${NC}"
            echo -e "${YELLOW}Creating generic build...${NC}"
            ;;
    esac
    
    echo -e "${GREEN}🎉 Build completed!${NC}"
    echo -e "${BLUE}Output directory: $DIST_DIR${NC}"
    ls -la "$DIST_DIR"
}

# Run main function
main "$@" 