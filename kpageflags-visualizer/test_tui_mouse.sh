#!/bin/bash

echo "Testing KPageFlags TUI with Mouse Support"
echo "========================================="
echo ""
echo "Features added:"
echo "- Click and drag to select an area for zooming"
echo "- Mouse scroll wheel for zoom in/out"
echo "- Visual selection feedback with highlighted cells"
echo "- Selection info display during drag"
echo "- ESC key to cancel selection"
echo ""
echo "Instructions:"
echo "1. Use mouse to click and drag over the grid to select an area"
echo "2. Release mouse button to zoom into the selected area"
echo "3. Use scroll wheel to zoom in/out"
echo "4. Use arrow keys to navigate"
echo "5. Press 'h' for full help"
echo "6. Press 'q' to quit"
echo ""
echo "Starting TUI mode..."
echo ""

# Run the TUI mode
sudo ./target/debug/kpageflags-visualizer --tui
