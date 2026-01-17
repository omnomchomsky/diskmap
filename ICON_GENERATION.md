# Icon Generation Instructions

## Current Setup

The project uses `treemap_icon.png` (1024x1024) from the project root as the main application icon.

**Icon locations:**
- **Source**: `treemap_icon.png` in project root (1024x1024 PNG)
- **GUI icon**: `crates/gui/icons/icon.png` (copy of treemap_icon.png)
- **Alternative SVG**: `crates/gui/icons/icon.svg` (custom SVG design)
- **Windows ICO**: `crates/gui/icons/icon.ico` (needs regeneration)

The Tauri configuration already points to `icons/icon.png`, so the treemap icon is now the default.

## Regenerating Icon Formats

The main PNG icon at 1024x1024 is sufficient for most purposes. However, if you need platform-specific formats:

### Option 1: Using Python Script (Included)

A Python script is included in the project root:

```bash
# Install Pillow if needed
pip install Pillow

# Run the script
python generate_icon.py
```

This will generate `crates/gui/icons/icon.ico` with standard Windows sizes (256, 128, 64, 48, 32, 16).

### Option 2: Using Tauri CLI (Optional)

If you have Tauri CLI installed:
```bash
cargo install tauri-cli  # One-time installation
cd crates/gui
cargo tauri icon icons/icon.png
```

### Option 3: Manual Generation for Windows ICO

If you need to manually generate a Windows .ico file:

**Using ImageMagick:**
```bash
magick convert treemap_icon.png -define icon:auto-resize=256,128,64,48,32,16 crates/gui/icons/icon.ico
```

**Using online tools:**
- https://convertio.co/png-ico/
- https://cloudconvert.com/png-to-ico

**Recommended sizes for .ico:** 16x16, 32x32, 48x48, 64x64, 128x128, 256x256

## Icon Design

The treemap_icon.png features a colorful treemap visualization that represents:
- Disk space allocation with proportional rectangles
- Hierarchical folder structure
- Color-coded file types and sizes
- Modern, professional appearance

This icon effectively communicates the app's purpose at a glance.
