#!/bin/bash

echo "Running PixelDrain application with debug output..."
echo "Please click on the Lists tab and then click 'Refresh Lists' to see the debug output"
echo "Press Ctrl+C to stop the application"
echo "----------------------------------------"

cargo run 2>&1 | tee debug_output.log 