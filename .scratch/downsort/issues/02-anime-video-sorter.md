# 02: Anime Video Sorter (Series Folder & Filename Preservation)

**What to build:** An anime sorting module that inspects video files in the download folder, identifies anime release patterns (fansub group tags, series title, and episode numbering), moves the files into show-specific folders (`~/Videos/Anime/<Show Name>/`) while preserving their original filenames, leaves non-anime/unrecognized videos untouched in downloads, and warns without overwriting if a file already exists at the destination.

**Blocked by:** 01: Core CLI Scaffolding, Configuration, and Active-Download Filter

**Status:** ready-for-human

## Acceptance criteria

- [ ] Inspects files with video extensions: `.mkv`, `.mp4`, `.avi`, `.webm`.
- [ ] Matches standard anime fansub filename patterns (e.g., `[ReleaseGroup] Series Name - 01 [1080p].mkv` or `[ReleaseGroup] Series Name - S01E02 (720p).mkv`).
- [ ] Extracts the clean show name (stripping brackets like `[SubsPlease]`, `[Erai-raws]`, resolution, and CRC checksums).
- [ ] Creates the series directory under the anime destination (`~/Videos/Anime/<Show Name>/`).
- [ ] Moves the video file into the series directory keeping the exact original filename intact.
- [ ] Skips moving and prints a warning if a file with the identical name already exists in the destination folder.
- [ ] Leaves generic or non-anime videos (e.g. `tutorial.mp4`, `screencast.mkv`, or personal clips without show patterns) untouched in the download folder.
- [ ] Respects `--dry-run`: logs what show folder would be created and what file would be moved without modifying the filesystem.
- [ ] Includes tests verifying:
  - Correct extraction of show titles from various fansub filename conventions.
  - Files are moved and directory created in target directory.
  - Destination collision skips file and preserves existing file.
  - Non-anime video filenames remain in source folder.

## Hints & Recommended Modules

### Recommended Python Modules & Strategies
- `re` (Regular Expressions): You can write a regex parser or clean up title brackets.
  - A common pattern for fansub titles is: `r"^\[(?P<group>[^\]]+)\]\s*(?P<title>.+?)\s*-\s*(?P<ep>\d+|S\d+E\d+).*?\.(?P<ext>mkv|mp4|webm)$"`
  - Alternatively, if you want a battle-tested parser, you can explore the lightweight Python package `anitopy-python` (`pip install anitopy-python`) which automatically parses title, episode, group, and quality from anime filenames into a dict.
- `shutil.move`: For moving files from source to destination.
- `pathlib.Path.mkdir(parents=True, exist_ok=True)`: Ensures parent show directories exist before moving.

### Edge Cases to Watch Out For
- Complex titles: Titles with colons, season numbers, or subtitles (e.g. `[SubsPlease] Frieren - Beyond Journey's End - 01 (1080p) [98234ABC].mkv`). Ensure your title extraction doesn't grab only the first word.
- Duplicate files: Check `destination_path.exists()` before calling `shutil.move`.
- Non-anime files with brackets: Make sure a non-anime file like `document[final].mp4` is not misclassified as an anime series named `final`.
