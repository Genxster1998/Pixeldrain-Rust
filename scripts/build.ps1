# PixelDrain Windows Build Script
# Run this script in PowerShell with: .\scripts\build.ps1

param(
    [switch]$SkipBuild,
    [switch]$SkipInstaller,
    [string]$Version = "0.1.0"
)

# Configuration
$AppName = "pixeldrain"
$Author = "Genxster1998"
$Description = "Upload and manage files with PixelDrain"
$License = "MIT"

# Directories
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$BuildDir = Join-Path $ProjectDir "target"
$DistDir = Join-Path $ProjectDir "dist"
$AssetsDir = Join-Path $ProjectDir "assets"

Write-Host "🚀 PixelDrain Windows Build Script" -ForegroundColor Blue
Write-Host "=================================" -ForegroundColor Blue

# Create necessary directories
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null

# Function to check if command exists
function Test-Command($cmdname) {
    return [bool](Get-Command -Name $cmdname -ErrorAction SilentlyContinue)
}

# Function to convert PNG to ICO
function Convert-PngToIco {
    param($PngFile, $IcoFile)
    
    if (Test-Command "magick") {
        Write-Host "Converting PNG to ICO..." -ForegroundColor Yellow
        magick $PngFile -resize 256x256 $IcoFile
    } else {
        Write-Host "Error: ImageMagick not found. Please install it to convert PNG to ICO." -ForegroundColor Red
        Write-Host "You can install it with: choco install imagemagick" -ForegroundColor Yellow
        exit 1
    }
}

# Function to build the application
function Build-Application {
    if (-not $SkipBuild) {
        Write-Host "Building application..." -ForegroundColor Blue
        cargo build --release
    }
}

# Function to create Windows installer
function Create-WindowsInstaller {
    if ($SkipInstaller) {
        Write-Host "Skipping installer creation..." -ForegroundColor Yellow
        return
    }
    
    Write-Host "Creating Windows installer..." -ForegroundColor Blue
    
    # Convert PNG to ICO if needed
    $IcoFile = Join-Path $AssetsDir "icon.ico"
    $PngFile = Join-Path $AssetsDir "dark-icon.png"
    
    if (-not (Test-Path $IcoFile)) {
        Convert-PngToIco $PngFile $IcoFile
    }
    
    # Create NSIS script
    $NsisScript = Join-Path $DistDir "installer.nsi"
    $ExePath = Join-Path $BuildDir "release\$AppName.exe"
    
    $NsisContent = @"
!include "MUI2.nsh"

; General
Name "PixelDrain"
OutFile "PixelDrain-$Version-Setup.exe"
InstallDir "`$PROGRAMFILES\PixelDrain"
InstallDirRegKey HKCU "Software\PixelDrain" ""

; Request application privileges
RequestExecutionLevel admin

; Interface Settings
!define MUI_ABORTWARNING
!define MUI_ICON "$IcoFile"
!define MUI_UNICON "$IcoFile"

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
    SetOutPath "`$INSTDIR"
    File "$ExePath"
    File "$IcoFile"
    
    ; Create uninstaller
    WriteUninstaller "`$INSTDIR\Uninstall.exe"
    
    ; Create start menu shortcut
    CreateDirectory "`$SMPROGRAMS\PixelDrain"
    CreateShortCut "`$SMPROGRAMS\PixelDrain\PixelDrain.lnk" "`$INSTDIR\$AppName.exe" "" "`$INSTDIR\icon.ico"
    CreateShortCut "`$SMPROGRAMS\PixelDrain\Uninstall.lnk" "`$INSTDIR\Uninstall.exe"
    
    ; Create desktop shortcut
    CreateShortCut "`$DESKTOP\PixelDrain.lnk" "`$INSTDIR\$AppName.exe" "" "`$INSTDIR\icon.ico"
    
    ; Registry information for add/remove programs
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "DisplayName" "PixelDrain"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "UninstallString" "`$INSTDIR\Uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "DisplayIcon" "`$INSTDIR\icon.ico"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "Publisher" "$Author"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "DisplayVersion" "$Version"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain" "NoRepair" 1
SectionEnd

; Uninstaller Section
Section "Uninstall"
    ; Remove files
    Delete "`$INSTDIR\$AppName.exe"
    Delete "`$INSTDIR\icon.ico"
    Delete "`$INSTDIR\Uninstall.exe"
    
    ; Remove shortcuts
    Delete "`$SMPROGRAMS\PixelDrain\PixelDrain.lnk"
    Delete "`$SMPROGRAMS\PixelDrain\Uninstall.lnk"
    RMDir "`$SMPROGRAMS\PixelDrain"
    Delete "`$DESKTOP\PixelDrain.lnk"
    
    ; Remove registry keys
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\PixelDrain"
    
    ; Remove install directory
    RMDir "`$INSTDIR"
SectionEnd
"@
    
    $NsisContent | Out-File -FilePath $NsisScript -Encoding UTF8
    
    # Build installer if makensis is available
    if (Test-Command "makensis") {
        makensis $NsisScript
        Write-Host "✅ Windows installer created: $DistDir\PixelDrain-$Version-Setup.exe" -ForegroundColor Green
    } else {
        Write-Host "makensis not found. NSIS script created: $NsisScript" -ForegroundColor Yellow
        Write-Host "Please install NSIS and run: makensis $NsisScript" -ForegroundColor Yellow
    }
}

# Function to create portable package
function Create-PortablePackage {
    Write-Host "Creating portable package..." -ForegroundColor Blue
    
    $PortableDir = Join-Path $DistDir "PixelDrain-Portable"
    New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null
    
    # Copy executable
    Copy-Item (Join-Path $BuildDir "release\$AppName.exe") $PortableDir
    Copy-Item (Join-Path $AssetsDir "icon.ico") $PortableDir
    
    # Create README
    $ReadmeContent = @"
PixelDrain Portable v$Version
============================

This is a portable version of PixelDrain that doesn't require installation.

Usage:
1. Double-click pixeldrain.exe to run
2. Set your API key via environment variable: PIXELDRAIN_API_KEY=your_key_here
3. Or configure it through the Settings tab in the application

For more information, visit: https://github.com/Genxster1998/PixelDrain

Copyright (c) 2025 $Author
"@
    
    $ReadmeContent | Out-File -FilePath (Join-Path $PortableDir "README.txt") -Encoding UTF8
    
    # Create ZIP archive
    $ZipFile = Join-Path $DistDir "PixelDrain-$Version-Portable.zip"
    Compress-Archive -Path $PortableDir\* -DestinationPath $ZipFile -Force
    
    Write-Host "✅ Portable package created: $ZipFile" -ForegroundColor Green
}

# Main build process
function Main {
    Build-Application
    Create-WindowsInstaller
    Create-PortablePackage
    
    Write-Host "🎉 Build completed!" -ForegroundColor Green
    Write-Host "Output directory: $DistDir" -ForegroundColor Blue
    Get-ChildItem $DistDir | Format-Table Name, Length, LastWriteTime
}

# Run main function
Main 