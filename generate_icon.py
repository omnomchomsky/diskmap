#!/usr/bin/env python3
"""
Generate Windows ICO file from PNG source.
Usage: python generate_icon.py
"""

from PIL import Image
import sys

def generate_ico(png_path, ico_path):
    """Generate ICO file with multiple sizes from PNG."""
    try:
        # Open the source PNG
        img = Image.open(png_path)

        # Define icon sizes (Windows standard sizes)
        sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)]

        # Create resized versions
        icon_images = []
        for size in sizes:
            resized = img.resize(size, Image.Resampling.LANCZOS)
            icon_images.append(resized)

        # Save as ICO with all sizes
        icon_images[0].save(
            ico_path,
            format='ICO',
            sizes=[(img.width, img.height) for img in icon_images]
        )

        print(f"✓ Successfully generated {ico_path}")
        print(f"  Sizes: {', '.join(f'{s[0]}x{s[1]}' for s in sizes)}")
        return True

    except ImportError:
        print("Error: PIL/Pillow is not installed.")
        print("Install it with: pip install Pillow")
        return False
    except FileNotFoundError:
        print(f"Error: Source file '{png_path}' not found.")
        return False
    except Exception as e:
        print(f"Error: {e}")
        return False

if __name__ == "__main__":
    png_source = "treemap_icon.png"
    ico_output = "crates/gui/icons/icon.ico"

    print(f"Generating Windows ICO from {png_source}...")
    success = generate_ico(png_source, ico_output)

    sys.exit(0 if success else 1)
