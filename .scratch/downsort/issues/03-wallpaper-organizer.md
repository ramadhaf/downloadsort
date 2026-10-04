# 03: Wallpaper Organizer with Aspect Ratio & Resolution Tolerance

**What to build:** An image categorization module that inspects downloaded image files, calculates their width, height, and aspect ratio ($width / height$), sorts high-resolution landscape images into desktop wallpapers and portrait images into mobile wallpapers, leaves square/low-res graphics untouched, and handles destination collisions safely.

**Blocked by:** 01: Core CLI Scaffolding, Configuration, and Active-Download Filter

**Status:** ready-for-human

## Acceptance criteria

- [ ] Inspects files with image extensions: `.png`, `.jpg`, `.jpeg`, `.webp`.
- [ ] Reads image pixel width and height without corrupting or loading full uncompressed image buffers into memory.
- [ ] Applies the tolerance bands agreed in the design:
  - **Desktop Wallpaper**: Aspect Ratio $> 1.2$ AND $width \ge 1280\text{px}$.
  - **Mobile Wallpaper**: Aspect Ratio $< 0.8$ AND $height \ge 1280\text{px}$.
  - **Ignored / Left in Downloads**: Aspect Ratio between $0.8$ and $1.2$ (square / near-square images like avatars and album covers), OR images with max dimension $< 1280\text{px}$ (icons, memes, web graphics).
- [ ] Moves qualifying images to:
  - Desktop: `~/Pictures/Wallpapers/Desktop/<filename>`
  - Mobile: `~/Pictures/Wallpapers/Mobile/<filename>`
- [ ] Skips moving and warns if a file with identical name already exists at destination.
- [ ] Respects `--dry-run`: previews the calculated aspect ratio, classification, and destination without moving files.
- [ ] Includes tests verifying:
  - Landscape image ($1920\times1080$, $2560\times1440$) categorizes as Desktop.
  - Portrait image ($1080\times2400$, $1170\times2532$) categorizes as Mobile.
  - Square image ($1000\times1000$) remains in source folder.
  - Low-resolution image ($800\times600$) remains in source folder.
  - Unrecognized or corrupted image formats fail gracefully without crashing the pipeline.

## Hints & Recommended Modules

### Recommended Python Modules
- `Pillow` (`from PIL import Image`): The standard library for image processing in Python (`pip install Pillow`).
  - To read dimensions without loading the entire image data into RAM:
    ```python
    with Image.open(image_path) as img:
        width, height = img.size
    ```
- `shutil.move`: For moving image files.

### Edge Cases to Watch Out For
- Division by zero: Guard against zero height or width (`height > 0`).
- Non-image files with image extensions: Catch `UnidentifiedImageError` or `OSError` if a file is truncated or corrupted, log a warning, and leave the file untouched.
- Collision check: Always verify `not target_path.exists()` before moving.
