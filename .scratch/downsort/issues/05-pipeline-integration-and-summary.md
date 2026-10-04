# 05: End-to-End Pipeline Integration & CLI Summary Reporting

**What to build:** The unified pipeline integrating the anime sorter, wallpaper organizer, and application installer into a seamless, high-performance CLI workflow. Provides a clean, formatted terminal summary reporting counts of processed, skipped, and ignored files, and comprehensive end-to-end test coverage across diverse download mixes.

**Blocked by:**
- 02: Anime Video Sorter (Series Folder & Filename Preservation)
- 03: Wallpaper Organizer with Aspect Ratio & Resolution Tolerance
- 04: Application Tarball Extractor & KDE Start Menu Integration

**Status:** ready-for-human

## Acceptance criteria

- [ ] Integrates all three sorters (Anime, Wallpaper, Application Tarball) into the main `downsort` scanning loop.
- [ ] Displays a formatted, easy-to-read summary at the end of execution:
  - Total files scanned
  - Anime videos moved (by show)
  - Wallpapers organized (Desktop vs Mobile counts)
  - Applications installed / updated
  - Files skipped (due to existing duplicate, low resolution, or active download)
- [ ] Exits with code `0` on successful completion.
- [ ] Handles uncaught errors on individual files gracefully without stopping the processing of remaining files (logs a warning for the failed file and continues).
- [ ] End-to-end integration test (`test_pipeline_e2e.py`):
  - Stages a mock downloads folder containing a mix of anime videos, desktop/mobile wallpapers, square avatars, application archives, source code archives, and `.crdownload` files.
  - Runs the pipeline in `--dry-run` mode and asserts that zero files were modified on disk.
  - Runs the pipeline in live mode and asserts:
    - Anime video moved to show folder with original name.
    - Wallpapers moved to Desktop and Mobile folders correctly.
    - App tarball extracted to applications folder and desktop file written.
    - Square avatar, source tarball, and `.crdownload` file remain in downloads.
    - Original app tarball is removed from downloads.

## Hints & Recommended Modules

### Recommended Python Modules
- `rich` (optional, for pretty terminal tables and colored summaries: `pip install rich`) or Python's standard `print` formatting.
- `dataclasses`: Clean data structures to track scan results (e.g. `ScanResult` holding counts and lists of actions).

### Edge Cases to Watch Out For
- Processing order: Ensure file dispatch checks the right handlers:
  1. Skip active downloads (`.crdownload`, `.part`, `.tmp`).
  2. If video extension -> Anime sorter.
  3. If image extension -> Wallpaper organizer.
  4. If `.tar.gz` / `.tgz` -> App installer.
  5. Other files -> Ignore / leave in downloads.
- Resilience: Wrap individual file handling in a `try...except Exception:` block so an unreadable image or corrupt archive doesn't crash the entire sort process for other files.
