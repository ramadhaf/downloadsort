# Spec: Downsort Download File Organizer

Status: ready-for-agent

## Problem Statement

The user's `~/Downloads` directory regularly accumulates a cluttered mix of downloaded files, including fansubbed anime video releases, desktop and mobile wallpapers, and portable application archives packaged as `.tar.gz`. Manually sorting anime into respective show libraries, checking image dimensions to categorize wallpapers between desktop and phone setups, and extracting Linux application tarballs to configure `.desktop` launcher shortcuts in KDE Plasma is repetitive, time-consuming, and error-prone. Without an automated tool, files either remain in a messy downloads folder or require tedious manual filing and launcher authoring.

## Solution

A fast, on-demand command-line tool (`downsort`) written in Python that automatically scans the user's downloads folder and categorizes files based on clear heuristics:
1. **Anime Videos**: Identifies fansub release patterns, creates a dedicated folder per show under the user's anime video directory (`~/Videos/Anime/<Show Name>/`), and moves the files while preserving original filenames. Unrecognized/generic videos are left alone.
2. **Wallpapers**: Inspects image dimensions and aspect ratios using strict tolerance bands. High-resolution landscape images are filed to desktop wallpapers, and portrait images to mobile wallpapers. Borderline, square, or low-resolution images are left untouched.
3. **Application Archives (`.tar.gz`)**: Inspects tarballs for executable binaries. True application archives are extracted to `~/Applications/<app-name>/`, integrated with KDE Plasma by generating standard XDG desktop launchers (`~/.local/share/applications/<app-name>.desktop`), symlinked to `~/.local/bin/`, and the original archive safely trashed. Non-application tarballs remain untouched.
4. **Safety & Transparency**: Provides non-destructive collision handling (skip & warn), automatically ignores incomplete downloads (`.crdownload`, `.part`), and includes a `--dry-run` flag to preview all operations.

## User Stories

1. As a user with multiple anime series downloaded, I want anime episodes to be automatically grouped into folders named after the show, so that my anime collection stays organized without manual sorting.
2. As a user downloading fansubbed anime, I want original filenames to be preserved when sorted, so that release group tags, episode numbering, and video quality information remain intact.
3. As a user downloading non-anime videos (such as tutorials or recordings), I want unrecognized videos to remain untouched in my downloads folder, so that personal or unrelated videos are not misplaced into anime folders.
4. As a user with already-sorted anime, I want `downsort` to skip moving duplicate files if a file with the same name already exists in the destination, so that I do not accidentally overwrite or redundantly copy videos.
5. As a wallpaper enthusiast, I want landscape wallpapers exceeding standard desktop resolutions to be automatically sorted into my desktop wallpaper folder, so that they are ready for my desktop background rotation.
6. As a mobile phone user downloading phone wallpapers on my PC, I want portrait wallpapers to be automatically placed in a mobile wallpaper folder, so that I can easily transfer them to my phone.
7. As a user downloading profile pictures, memes, and icons, I want square and near-square images to be ignored by the wallpaper sorter, so that general pictures do not pollute my wallpaper collections.
8. As a user downloading web graphics, I want low-resolution images below minimum resolution thresholds to remain in my downloads directory, so that poor-quality images are not treated as wallpapers.
9. As a Linux user downloading standalone software in `.tar.gz` format, I want application archives to be extracted directly into an applications folder, so that portable apps have a consistent installation home.
10. As a KDE Plasma user, I want extracted portable applications to automatically have a `.desktop` shortcut generated in my applications menu, so that I can launch them directly from the KDE Kickoff start menu.
11. As a Linux user, I want the tool to detect and link the application icon from the archive, so that the start menu entry displays the proper application artwork.
12. As a Linux user, I want the tool to fall back to a standard application icon if no icon image is bundled in the archive, so that the launcher entry remains valid and visually consistent.
13. As a terminal user, I want the main binary of extracted applications to be symlinked into my user bin directory, so that I can launch installed applications from my shell.
14. As a user downloading non-application archives (such as code repositories, backups, or document bundles), I want archives without executables to be left untouched in downloads, so that arbitrary tarballs are not misconfigured as applications.
15. As a user updating existing portable software, I want re-downloading an existing app tarball to cleanly upgrade the application in place, so that my start menu shortcut continues working with the newest version.
16. As a cautious user, I want extracted archives to be sent to the system Trash rather than permanently deleted, so that I can recover the original archive if an extraction fails or is unwanted.
17. As an active downloader, I want files currently being downloaded by my browser to be ignored, so that incomplete files are never moved or corrupted.
18. As a user testing new download batches, I want a dry-run flag to simulate what would be moved or extracted, so that I can verify all operations before modifying any files.
19. As a user customizing my storage setup, I want default directory locations and resolution thresholds to be configurable via a configuration file, so that I can adapt the tool to custom paths.
20. As a CLI user, I want a clear summary output upon running the tool, so that I can see exactly which files were organized, extracted, skipped, or ignored.

## Implementation Decisions

- **Architectural Separation**: The system will be divided into modular domain handlers coordinated by a central pipeline:
  - An anime identification module that parses filenames according to release conventions (bracketed group tags, series title, episode index) and determines the destination series folder.
  - An image categorization module that reads dimensions and evaluates aspect ratio bands against configurable tolerances (Ratio > 1.2 and width >= 1280 for desktop; Ratio < 0.8 and height >= 1280 for mobile).
  - An archive inspection and application installer module that reads tarball headers to identify executable binaries and icons, extracts to application storage, generates standard freedesktop XDG desktop entries, and handles safe disposal via desktop trash APIs.
  - A core organizer pipeline that discovers eligible files, filters out active downloads, delegates to the appropriate handlers, and records execution metrics.
- **Interface & Configuration**:
  - The tool provides a clean CLI entrypoint supporting standard execution, a `--dry-run` flag, and configuration inspection.
  - User configuration persists in a structured format under the user configuration directory, defaulting to standard user directories for downloads, videos, pictures, and applications.
- **Archive Extraction Semantics**:
  - Archives must be inspected safely without extracting arbitrary tarballs. If an archive contains at least one executable file or shell script matching the package identity, it qualifies as an application.
  - Generated desktop files conform strictly to the XDG Desktop Entry Specification, populating Name, Exec, Icon, Type, and Categories.
  - Archive removal utilizes the desktop trash protocol rather than destructive file unlinking.
- **Collision Strategy**:
  - Existing files at media destinations are never overwritten silently. When a collision occurs, the operation is skipped and reported to the user.

## Testing Decisions

- **Testing Principles**: Tests will focus exclusively on external behavioral outcomes: file locations, directory creations, desktop entry validity, and dry-run invariance. Internal private functions and parsing heuristics are tested through the public interface.
- **Testing Seam**: The highest and only primary seam is the CLI pipeline entrypoint. The test runner prepares a temporary mock filesystem (mocking downloads, videos, pictures, and applications directories) with sample video files, images of various dimensions, valid application tarballs, source-only tarballs, and active download markers, then invokes the organizer runner.
- **Behavioral Assertions**:
  - Verify that eligible anime files land in series-specific directories under the anime root with identical filenames.
  - Verify that unrecognized videos remain in the mock download folder.
  - Verify that landscape high-resolution images land in desktop wallpapers, portrait images land in mobile wallpapers, and square/low-res images remain in downloads.
  - Verify that app tarballs are extracted to the applications directory, valid `.desktop` files are generated, executable permissions are preserved, and original tarballs are moved to trash.
  - Verify that non-app tarballs remain in downloads.
  - Verify that dry-run mode leaves all files in their initial state without modifying the filesystem.

## Out of Scope

- Background daemon, automatic file watching (`inotify`), or systemd user services (focus is purely on the on-demand CLI tool).
- Video metadata tagging, transcoding, or fetching metadata from online anime databases (AniList, MyAnimeList).
- Interactive GUI or TUI configuration menus.
- Complex package management, uninstallation tracking, or dependency resolution for extracted `.tar.gz` applications.
- Multi-part archive formats (`.rar`, `.7z`, `.tar.xz`, multi-volume zip).

## Further Notes

- The tool is designed to run seamlessly in modern Linux environments adhering to freedesktop.org XDG standards (specifically tested on Arch / EndeavourOS with KDE Plasma).
- Dependencies should remain minimal, leveraging Python 3 standard libraries wherever possible with standard image handling.
