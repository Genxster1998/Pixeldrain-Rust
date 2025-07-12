#!/bin/bash

# PixelDrain API Debug Test Script
# This script helps test the API endpoints to verify they work correctly

echo "🐛 PixelDrain API Debug Test"
echo "============================"

# Check if API key is set
if [ -z "$PIXELDRAIN_API_KEY" ]; then
    echo "❌ PIXELDRAIN_API_KEY environment variable not set"
    echo "Please set your API key: export PIXELDRAIN_API_KEY='your_api_key_here'"
    exit 1
fi

echo "✅ API Key found: ${PIXELDRAIN_API_KEY:0:8}..."

# Test 1: Get user files
echo ""
echo "📋 Test 1: Getting user files..."
response=$(curl -s -H "Authorization: Basic $(echo -n ":$PIXELDRAIN_API_KEY" | base64)" \
    "https://pixeldrain.com/api/user/files")

if [ $? -eq 0 ]; then
    echo "✅ Success! Response:"
    echo "$response" | jq '.' 2>/dev/null || echo "$response"
else
    echo "❌ Failed to get user files"
fi

# Test 2: Get user info
echo ""
echo "👤 Test 2: Getting user info..."
response=$(curl -s -H "Authorization: Basic $(echo -n ":$PIXELDRAIN_API_KEY" | base64)" \
    "https://pixeldrain.com/api/user")

if [ $? -eq 0 ]; then
    echo "✅ Success! Response:"
    echo "$response" | jq '.' 2>/dev/null || echo "$response"
else
    echo "❌ Failed to get user info"
fi

# Test 3: Test file info endpoint (if we have a file ID)
echo ""
echo "📄 Test 3: Testing file info endpoint..."
echo "Note: This requires a valid file ID. You can get one from the files list above."

# If you have a file ID, uncomment and modify this:
# FILE_ID="your_file_id_here"
# if [ ! -z "$FILE_ID" ]; then
#     response=$(curl -s -H "Authorization: Basic $(echo -n ":$PIXELDRAIN_API_KEY" | base64)" \
#         "https://pixeldrain.com/api/file/$FILE_ID")
#     if [ $? -eq 0 ]; then
#         echo "✅ Success! Response:"
#         echo "$response" | jq '.' 2>/dev/null || echo "$response"
#     else
#         echo "❌ Failed to get file info"
#     fi
# else
#     echo "⚠️  No file ID provided for testing"
# fi

echo ""
echo "🔧 Debug Tips:"
echo "1. Check the debug panel in the app (click the 🐛 button)"
echo "2. Look for error messages in the UI"
echo "3. Check the console output for any errors"
echo "4. Verify your API key is correct at https://pixeldrain.com/user/settings"
echo ""
echo "🚀 Run the app with: cargo run" 