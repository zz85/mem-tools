# Mouse Zoom Features Implementation

## Overview
Enhanced the KPageFlags TUI visualizer with comprehensive mouse support for interactive grid exploration.

## New Features Implemented

### 🖱️ Mouse Selection & Zoom
- **Click and Drag Selection**: Users can click and drag to select rectangular areas in the grid
- **Visual Selection Feedback**: Selected cells are highlighted with inverted colors during drag
- **Zoom to Selection**: Releasing the mouse button zooms into the selected area
- **Selection Info Display**: Shows selection dimensions and coordinates during drag

### 🔄 Mouse Scroll Support
- **Scroll Wheel Zoom**: Mouse wheel up/down for zoom in/out
- **Smooth Zoom Levels**: Zoom levels from 0.1x to 10x with smooth transitions

### ⌨️ Enhanced Keyboard Controls
- **ESC Key**: Cancel current selection
- **Existing Controls**: All previous keyboard shortcuts maintained

## Technical Implementation

### Code Changes Made

1. **Enhanced AppState Structure** (`src/tui.rs`)
   - Added mouse selection state tracking
   - Added grid area storage for coordinate mapping
   - Added selection start/end coordinates

2. **Mouse Event Handling**
   - Integrated crossterm mouse events
   - Added mouse event processing in main event loop
   - Implemented coordinate mapping between mouse and grid

3. **Selection Logic**
   - `handle_mouse_event()`: Processes all mouse interactions
   - `zoom_to_selection()`: Calculates zoom and offset for selected area
   - `is_cell_in_selection()`: Determines if cells should be highlighted

4. **Visual Feedback**
   - Selection overlay rendering during drag operations
   - Highlighted cells with inverted colors
   - Selection info display with dimensions and coordinates

5. **Grid Rendering Updates**
   - Modified `render_grid()` to support selection visualization
   - Added selection overlay rendering
   - Updated grid title to indicate mouse functionality

## User Experience Improvements

### Intuitive Interaction
- **Natural Mouse Behavior**: Click-drag-release pattern familiar to users
- **Visual Feedback**: Immediate visual response during selection
- **Smooth Zoom**: Calculated zoom levels that fit selection to screen

### Enhanced Navigation
- **Precise Area Selection**: Users can select exact areas of interest
- **Multi-level Zoom**: Combine mouse selection with scroll wheel for fine control
- **Easy Reset**: Home key or manual navigation to explore different areas

### Accessibility
- **Keyboard Fallback**: All functionality available via keyboard
- **Clear Instructions**: Updated help text with mouse controls
- **Status Indicators**: Footer shows selection status and zoom level

## Usage Examples

### Basic Mouse Zoom Workflow
1. Launch TUI: `sudo cargo run -- --tui`
2. Click and drag over interesting area in the grid
3. Release to zoom into selected area
4. Use scroll wheel for fine zoom adjustment
5. Use arrow keys to navigate around zoomed area
6. Press Home to reset view

### Advanced Exploration
1. Start with overview of all pages
2. Use category filters (1-8) to focus on specific flag types
3. Mouse-select dense areas for detailed examination
4. Combine with statistics panel (s) for data analysis
5. Use refresh (r) to update data while exploring

## Files Modified

- `src/tui.rs`: Main TUI implementation with mouse support
- `README.md`: Updated documentation with mouse features
- `Cargo.toml`: Dependencies already included crossterm with mouse support

## Testing

### Test Scripts Created
- `test_tui_mouse.sh`: Basic mouse functionality test
- `demo_tui_features.sh`: Comprehensive feature demonstration

### Manual Testing Checklist
- ✅ Mouse click and drag selection
- ✅ Visual selection feedback
- ✅ Zoom to selection functionality
- ✅ Mouse scroll wheel zoom
- ✅ ESC key selection cancellation
- ✅ Coordinate mapping accuracy
- ✅ Selection info display
- ✅ Integration with existing keyboard controls

## Performance Considerations

### Efficient Rendering
- Selection highlighting only during active drag
- Minimal overhead for mouse event processing
- Responsive grid updates during zoom operations

### Memory Usage
- Selection state stored efficiently
- No additional data structures for mouse support
- Reuses existing grid rendering infrastructure

## Future Enhancement Possibilities

### Additional Mouse Features
- Right-click context menus
- Double-click for quick zoom
- Mouse hover for page info tooltips
- Drag-to-pan functionality

### Advanced Selection
- Multiple selection areas
- Selection history/bookmarks
- Export selected area data
- Selection-based filtering

## Conclusion

The mouse zoom implementation provides an intuitive and powerful way to explore kernel page flags data. The combination of visual selection feedback, smooth zooming, and integration with existing keyboard controls creates a comprehensive interactive experience for analyzing memory page information.
