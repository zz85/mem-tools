#!/bin/bash

echo "KPageFlags TUI with Mouse Zoom - Feature Demo"
echo "============================================="
echo ""
echo "🖱️  NEW MOUSE FEATURES:"
echo "   • Click and drag to select areas for zooming"
echo "   • Visual selection feedback with highlighted cells"
echo "   • Mouse scroll wheel for zoom in/out"
echo "   • Selection info display during drag operations"
echo ""
echo "⌨️  KEYBOARD CONTROLS:"
echo "   • Arrow keys: Navigate the grid"
echo "   • +/- : Zoom in/out"
echo "   • Home: Reset view"
echo "   • ESC: Cancel selection"
echo "   • h: Toggle help"
echo "   • s: Toggle statistics"
echo "   • r: Refresh data"
echo "   • q: Quit"
echo ""
echo "🎯 FILTERING:"
echo "   • 1-8: Filter by flag categories"
echo "   • 0: Clear filter"
echo ""
echo "📊 VISUALIZATION:"
echo "   • Color-coded symbols for different flag types"
echo "   • Real-time statistics panel"
echo "   • Progressive data loading with progress bar"
echo ""
echo "🔍 HOW TO USE MOUSE ZOOM:"
echo "   1. Click and hold left mouse button"
echo "   2. Drag to select the area you want to examine"
echo "   3. Release to zoom into the selected area"
echo "   4. Use scroll wheel for fine zoom control"
echo ""
echo "Starting TUI mode..."
echo "Press any key to continue..."
read -n 1 -s

# Check if we have sudo access
if [ "$EUID" -ne 0 ]; then
    echo "Note: Running without sudo - some pages may not be accessible"
    echo "For full functionality, run: sudo $0"
    echo ""
fi

# Run the TUI mode
if [ "$EUID" -eq 0 ]; then
    ./target/debug/kpageflags-visualizer --tui
else
    echo "Attempting to run with sudo..."
    sudo ./target/debug/kpageflags-visualizer --tui
fi
