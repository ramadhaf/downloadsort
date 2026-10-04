# 04: Application Tarball Extractor & KDE Start Menu Integration

**What to build:** An application installer for `.tar.gz` packages. It inspects archive members for executable binaries before extracting. If it contains an application, it extracts to `~/Applications/<app-name>/`, finds the primary executable and icon, creates an XDG `.desktop` launcher in `~/.local/share/applications/` so it appears in KDE Kickoff, symlinks the binary to `~/.local/bin/`, and trashes the original archive. If the archive has no executables, it leaves it untouched in Downloads.

**Blocked by:** 01: Core CLI Scaffolding, Configuration, and Active-Download Filter

**Status:** ready-for-human

## Acceptance criteria

- [ ] Inspects `.tar.gz` and `.tgz` archives in the downloads folder.
- [ ] Safe inspection without pre-extracting: reads the tarball table of contents to check if any file has executable bits or is a Linux ELF binary / shell script.
- [ ] If no executable is present (e.g. source tarball, doc archive, zip of text files): leaves the archive untouched in Downloads.
- [ ] If executables are found:
  - Derives application name from archive filename (e.g. `discord-0.0.28.tar.gz` -> `discord`, `postman-linux-x64.tar.gz` -> `postman`).
  - Target directory: `~/Applications/<app-name>/`. If target directory already exists, updates/replaces in place.
  - Safely extracts archive into `~/Applications/<app-name>/`.
  - Identifies the main executable (matching the app name or top-level runnable file/script).
  - Searches for an icon (`.png` or `.svg`) in the extracted package. If not found, falls back to `application-x-executable`.
  - Creates a valid Freedesktop XDG `.desktop` file at `~/.local/share/applications/<app-name>.desktop`:
    ```ini
    [Desktop Entry]
    Version=1.0
    Type=Application
    Name=<Formatted App Name>
    Exec=<Absolute path to executable>
    Icon=<Path to icon or application-x-executable>
    Terminal=false
    Categories=Utility;Application;
    StartupNotify=true
    ```
  - Creates a symlink in `~/.local/bin/<app-name>` pointing to the executable.
  - Moves the original `.tar.gz` file to the system Trash using `send2trash` or `gio trash` (never permanent unrecoverable deletion).
- [ ] Respects `--dry-run`: previews the app name, extracted destination, binary, icon, and desktop file path without extracting or trashing files.
- [ ] Includes tests verifying:
  - Tarball containing executable is extracted and `.desktop` entry created with correct paths.
  - Tarball containing only `.txt` or `.md` files is skipped and left in source folder.
  - Existing app directory is updated in place.
  - Original archive is removed/trashed after successful installation.

## Hints & Recommended Modules

### Recommended Python Modules
- `tarfile`: Standard library for inspecting and extracting `.tar.gz` archives.
  - Use `tarfile.open(archive_path, "r:gz")`.
  - Inspect members using `tar.getmembers()`. Check member modes with `member.mode & 0o111 != 0` (executable flag) or inspect filename extensions like `.sh`.
  - For safe extraction (avoiding Zip Slip / directory traversal attacks), ensure paths don't start with `/` or contain `..`. (Python 3.12+ supports `filter='data'` in `tar.extractall`).
- `send2trash` (`pip install send2trash`): Cross-platform, freedesktop-compliant trash library that moves files to `~/.local/share/Trash` on Linux. Alternatively, use `subprocess.run(["gio", "trash", str(archive_path)])`.
- `os.symlink`: For creating `~/.local/bin/<app-name>` symlink.

### Edge Cases to Watch Out For
- Tarball structure: Some tarballs extract everything into a root folder `myapp/`, while others dump files directly without a root directory. When extracting, ensure files are neatly contained in `~/Applications/<app-name>/`.
- Executable finding heuristic: Look for a file whose name matches `<app-name>` or has executable permissions, or a `.sh` launcher (e.g. `launch.sh`, `start.sh`, `run.sh`).
- Icon finding: Search recursively for `*.png` or `*.svg` inside the extracted folder. Pick the highest resolution icon if multiple exist, or the one matching the app name.
