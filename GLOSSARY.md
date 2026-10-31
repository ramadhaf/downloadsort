# Downsort

`downsort` organizes a cluttered downloads directory by classifying candidate files into media libraries and portable application installations.

## Language

**Source Directory**:
The directory scanned for candidate files to organize (defaults to `~/Downloads`).
_Avoid_: Input folder, inbox, watch folder

**Category Destination**:
One of the four domain-specific target directories where classified items are placed: Anime Directory, Desktop Wallpapers Directory, Mobile Wallpapers Directory, and Applications Directory.
_Avoid_: Output folder, target root, export folder

**Active Download**:
An incomplete file still being written by a browser or downloader (`*.crdownload`, `*.part`, `*.tmp`) that must be left untouched during a scan.
_Avoid_: Temp file, partial download

**Fansub Release**:
A video file whose filename encodes a release group tag, series title, and episode number, allowing it to be filed into a show-specific subfolder under the Anime Directory with its original filename preserved (labeled `"Anime"` in scan reports and GUI tables).
_Avoid_: Anime clip, raw video

**Desktop Wallpaper**:
A high-resolution landscape image exceeding the configured minimum aspect ratio and width thresholds, filed to the Desktop Wallpapers Directory.

**Mobile Wallpaper**:
A high-resolution portrait image below the configured maximum aspect ratio and exceeding the minimum height threshold, filed to the Mobile Wallpapers Directory.

**Application Archive**:
A compressed tarball (`.tar.gz` or `.tgz`) containing at least one executable binary or runnable script, which is extracted to the Applications Directory, registered in the desktop launcher menu, symlinked to the user binary path, and moved to the system Trash.
_Avoid_: Source tarball, package bundle

**Collision**:
A condition where a file with the same name already exists at a Category Destination; resolved non-destructively by skipping the move and warning the user.
_Avoid_: Conflict, duplicate overwrite

**Dry Run Preview**:
A non-destructive simulation of a scan that reports every planned classification, destination path, and skipped item without modifying the filesystem.
_Avoid_: Test run, simulation mode

**Session Override**:
A temporary Source Directory or Category Destination path chosen for the current preview or sort run without overwriting the saved default configuration.
_Avoid_: Temporary config, ephemeral profile

