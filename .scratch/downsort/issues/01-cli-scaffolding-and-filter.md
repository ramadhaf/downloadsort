# 01: Core CLI Scaffolding, Configuration, and Active-Download Filter

**What to build:** A runnable command-line interface `downsort` that loads or initializes default configuration, scans a designated source directory (defaults to `~/Downloads`), identifies and ignores active browser downloads, supports a `--dry-run` preview flag, and reports the scanned files.

**Blocked by:** None (can start immediately).

**Status:** ready-for-human

## Acceptance criteria

- [ ] CLI executable accepts flags: `--dry-run` (simulate without modifying filesystem), `--config` (show or specify config file path), and an optional `--source` directory path.
- [ ] Automatically creates and loads a default configuration file at `~/.config/downsort/config.json` on first run if it does not already exist.
- [ ] Default configuration contains standard paths:
  - Source directory: `~/Downloads`
  - Anime directory: `~/Videos/Anime`
  - Desktop wallpapers: `~/Pictures/Wallpapers/Desktop`
  - Mobile wallpapers: `~/Pictures/Wallpapers/Mobile`
  - Applications directory: `~/Applications`
  - Desktop launcher directory: `~/.local/share/applications`
- [ ] Active and temporary browser downloads (`*.crdownload`, `*.part`, `*.tmp`, `.*`) are safely filtered out and ignored.
- [ ] Includes automated tests using `pytest` verifying:
  - Active download files are excluded from processing.
  - Default configuration is written and read correctly.
  - `--dry-run` flag is parsed and recorded in the runner state.

## Hints & Recommended Modules

### Recommended Python Modules
- `pathlib.Path`: Use this for all path manipulations (e.g. `path.expanduser()`, `path.glob()`, `path.suffix`, `path.exists()`, `path.mkdir(parents=True, exist_ok=True)`). Avoid string concatenation for paths.
- `argparse`: Standard library module for building CLI interfaces with help messages and optional arguments (`parser.add_argument("--dry-run", action="store_true")`).
- `json`: Standard library for reading and writing `config.json`.
- `pytest`: For writing test functions. Use the built-in `tmp_path` fixture to create mock download folders without touching your real files.

### Edge Cases to Watch Out For
- Tilde expansion: Paths like `~/Downloads` must be expanded with `.expanduser()` before checking `.exists()`.
- Hidden files: Downloads often contain dotfiles (`.DS_Store`, `.git`, `.stfolder`) which should be skipped.
- In-progress downloads: Browsers like Chrome/Brave create `.crdownload` extensions, and Firefox creates `.part` extensions. Make sure your filter checks the end of the filename (or suffix).
