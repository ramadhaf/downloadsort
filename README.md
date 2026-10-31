# Downsort — Fast Linux Downloads Organizer (Anime, Wallpapers & Portable Apps) in Rust + PyQt6

`downsort` is a fast, zero-dependency Rust CLI engine with a `PyQt6` desktop GUI for automatically organizing your `~/Downloads` folder on Linux.

It classifies and sorts downloaded files into clean destinations:
- **Fansub Releases (`Anime`)**: Detects fansub and episodic video filenames (e.g., `[SubsPlease] Frieren - 01 [1080p].mkv`, `Show Name - S01E02.mp4`), strips release group tags, episode markers, resolutions, and CRC32 hashes, and moves episodes into `~/Videos/Anime/<Show Name>/`.
- **Desktop & Mobile Wallpapers**: Reads PNG, JPEG, and WebP dimensions directly from binary file headers (with ImageMagick `identify` fallback) without decoding full pixel buffers:
  - **Desktop Wallpaper**: Aspect ratio `> 1.2` and width `>= 1280` → `~/Pictures/Wallpapers/Desktop/`
  - **Mobile Wallpaper**: Aspect ratio `< 0.8` and height `>= 1280` → `~/Pictures/Wallpapers/Mobile/`
  - Near-square (`0.8..=1.2`) or small images (`max(width, height) < 1280`) are left untouched.
- **Application Archives (`.tar.gz`, `.tgz`)**: Safely inspects archive contents before extracting (rejecting path traversal or unsafe symlink/hardlink targets), extracts portable Linux applications to `~/Applications/<app-name>/`, links the main binary to `~/.local/bin/<app-name>`, generates an XDG `.desktop` launcher in `~/.local/share/applications/<app-name>.desktop`, and moves the original archive to the Freedesktop Trash (`$XDG_DATA_HOME/Trash` or `~/.local/share/Trash`).
- **Active Downloads & Unmatched Files**: Safely ignores in-progress browser/torrent files (`*.crdownload`, `*.part`, `*.tmp`), hidden dotfiles, and non-matching files, and never overwrites existing destination files on collision.

---

## Build & Installation

### Requirements
- **Rust toolchain** (`cargo`, `rustc` — zero third-party crate dependencies)
- **System utilities**: `tar` (required for `.tar.gz`/`.tgz` archives), optional `gio` (for trash fallback) and `identify` (ImageMagick fallback)
- **Desktop GUI (`--gui`)**: `python3` with `PyQt6` installed (`python3 -c "import PyQt6"`)

### Build Release Binary
```bash
cargo build --release
```

### Install to `~/.local/bin`
```bash
mkdir -p ~/.local/bin
cp ./target/release/downsort ~/.local/bin/downsort
```

---

## Usage

### 1. Desktop GUI (`PyQt6`)
Launch the graphical organizer window:
```bash
downsort --gui
```
- **Folder Pickers**: View or change the **Source Directory** and the 4 **Category Destination** directories using the **[Browse...]** buttons.
- **[Preview / Refresh]**: Runs a non-destructive **Dry Run Preview** (`--dry-run --json`) and displays every file in your Source Directory with its `Category`, `Status / Reason`, and `Destination`.
- **[Save Config]**: Persists the current directory paths to `~/.config/downsort/config.json`.
- **[Sort Now]**: Executes a live sort run and refreshes the table and summary bar.

### 2. Command-Line Interface (CLI)

```bash
# Preview all planned actions without modifying any files
downsort --dry-run

# Execute a live sort run using ~/.config/downsort/config.json
downsort

# Override input or output directories for a single run (Session Overrides)
downsort --dry-run \
  --source ~/Downloads \
  --anime-dir ~/Videos/Anime \
  --desktop-wallpapers-dir ~/Pictures/Wallpapers/Desktop \
  --mobile-wallpapers-dir ~/Pictures/Wallpapers/Mobile \
  --apps-dir ~/Applications

# Persist directory overrides to ~/.config/downsort/config.json
downsort --source ~/Downloads --anime-dir ~/Videos/Anime --save-config

# Print the active configuration as JSON
downsort --show-config

# Output structured machine-readable JSON for a dry-run or live scan
downsort --dry-run --json

# Use a custom config file path
downsort --config /path/to/config.json --dry-run
```

---

## Configuration

On first run, `downsort` initializes `~/.config/downsort/config.json` with the following defaults:

```json
{
  "source": "~/Downloads",
  "anime": "~/Videos/Anime",
  "desktop_wallpapers": "~/Pictures/Wallpapers/Desktop",
  "mobile_wallpapers": "~/Pictures/Wallpapers/Mobile",
  "applications": "~/Applications",
  "desktop_launchers": "~/.local/share/applications",
  "bin_dir": "~/.local/bin",
  "desktop_min_ratio": 1.2,
  "mobile_max_ratio": 0.8,
  "min_dimension": 1280
}
```

---

## Development & Testing

Run the integration and headless `PyQt6` (`QT_QPA_PLATFORM=offscreen`) test suite:
```bash
cargo test
```

Architecture & domain documentation:
- [`GLOSSARY.md`](GLOSSARY.md) — Ubiquitous domain vocabulary
- [`docs/adr/0001-rust-cli-engine-with-pyqt6-gui.md`](docs/adr/0001-rust-cli-engine-with-pyqt6-gui.md) — Architecture Decision Record for the Rust CLI engine + `PyQt6` GUI split
