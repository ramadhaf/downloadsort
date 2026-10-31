use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestEnv {
    root: PathBuf,
    config_path: PathBuf,
    source: PathBuf,
    anime: PathBuf,
    desktop_wp: PathBuf,
    mobile_wp: PathBuf,
    apps: PathBuf,
    launchers: PathBuf,
    bin_dir: PathBuf,
    xdg_data: PathBuf,
}

impl TestEnv {
    fn new(prefix: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "downsort_test_{}_{}_{}",
            prefix,
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("downloads");
        fs::create_dir_all(&source).unwrap();

        let env = Self {
            config_path: root.join("config.json"),
            source,
            anime: root.join("anime"),
            desktop_wp: root.join("desktop_wp"),
            mobile_wp: root.join("mobile_wp"),
            apps: root.join("apps"),
            launchers: root.join("launchers"),
            bin_dir: root.join("bin"),
            xdg_data: root.join("xdg_data"),
            root,
        };
        env.write_config();
        env
    }

    fn run_cli<I, S>(&self, args: I) -> Result<String, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let exe = env!("CARGO_BIN_EXE_downsort");
        let output = std::process::Command::new(exe)
            .env("HOME", &self.root)
            .env("XDG_DATA_HOME", &self.xdg_data)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to spawn downsort: {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    fn write_config(&self) {
        let json = format!(
            "{{\n  \"source\": \"{}\",\n  \"anime\": \"{}\",\n  \"desktop_wallpapers\": \"{}\",\n  \"mobile_wallpapers\": \"{}\",\n  \"applications\": \"{}\",\n  \"desktop_launchers\": \"{}\",\n  \"bin_dir\": \"{}\",\n  \"desktop_min_ratio\": 1.2,\n  \"mobile_max_ratio\": 0.8,\n  \"min_dimension\": 1280\n}}\n",
            self.source.display(),
            self.anime.display(),
            self.desktop_wp.display(),
            self.mobile_wp.display(),
            self.apps.display(),
            self.launchers.display(),
            self.bin_dir.display(),
        );
        fs::write(&self.config_path, json).unwrap();
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn snapshot_dir(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_str().unwrap().to_string(),
                fs::read(&p).unwrap(),
            )
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn initializes_default_config_and_prints_with_show_config() {
    let env = TestEnv::new("config_init");
    let custom_cfg = env.root.join("nested").join("config.json");

    // In dry-run without --show-config, missing config should not be written to disk
    let dry_out = downsort::run_cli([
        "--config",
        custom_cfg.to_str().unwrap(),
        "--source",
        env.source.to_str().unwrap(),
        "--dry-run",
    ])
    .expect("dry-run should succeed without writing missing config");
    assert!(!custom_cfg.exists(), "dry-run must not create config.json on disk");
    assert!(dry_out.contains("Dry run"));

    // With --show-config, initializes and prints default config
    let output = downsort::run_cli([
        "--config",
        custom_cfg.to_str().unwrap(),
        "--show-config",
    ])
    .expect("CLI should succeed");

    assert!(custom_cfg.exists(), "config.json should be created");
    let saved = fs::read_to_string(&custom_cfg).unwrap();
    assert!(saved.contains("\"source\""));
    assert!(saved.contains("\"anime\""));
    assert!(saved.contains("\"desktop_wallpapers\""));
    assert!(saved.contains("\"mobile_wallpapers\""));
    assert!(saved.contains("\"applications\""));
    assert!(saved.contains("\"desktop_launchers\""));
    assert!(saved.contains("\"bin_dir\""));
    assert!(saved.contains("\"desktop_min_ratio\": 1.2"));
    assert!(saved.contains("\"mobile_max_ratio\": 0.8"));
    assert!(saved.contains("\"min_dimension\": 1280"));
    assert!(output.contains("desktop_min_ratio"));
}

#[test]
fn filters_active_downloads_and_dotfiles_and_respects_source_override() {
    let env = TestEnv::new("filter_scan");

    fs::write(env.source.join("movie.crdownload"), b"partial").unwrap();
    fs::write(env.source.join("archive.tar.gz.part"), b"partial").unwrap();
    fs::write(env.source.join("temp.tmp"), b"temp").unwrap();
    fs::write(env.source.join(".hidden_file"), b"hidden").unwrap();
    fs::write(env.source.join("notes.txt"), b"complete").unwrap();

    let output = downsort::run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--source",
        env.source.to_str().unwrap(),
        "--dry-run",
    ])
    .expect("CLI should succeed");

    assert!(output.contains("Dry run"), "Output should indicate dry-run mode: {output}");
    assert!(
        output.contains("Total files scanned: 4"),
        "Should count 4 non-hidden entries scanned: {output}"
    );
    assert!(
        output.contains("Files skipped: 4"),
        "Should report 4 files skipped (3 active downloads + 1 unrecognized): {output}"
    );
    assert!(env.source.join("movie.crdownload").exists());
    assert!(env.source.join("archive.tar.gz.part").exists());
    assert!(env.source.join("temp.tmp").exists());
    assert!(env.source.join(".hidden_file").exists());
    assert!(env.source.join("notes.txt").exists());
}

#[test]
fn sorts_anime_videos_preserves_filenames_handles_collisions_and_dry_run() {
    let env = TestEnv::new("anime_sort");

    let ep1 = "[SubsPlease] Frieren - Beyond Journey's End - 01 (1080p) [98234ABC].mkv";
    let ep2 = "[Erai-raws] Dandadan - S01E02 [720p].MP4";
    let ep3 = "[HorribleSubs] Mob Psycho 100 - 12 [1080p].webm";
    let ep4 = "[SubsPlease] Frieren - 01 - The Journey's End [1080p].mkv";
    let unsafe_dotdot = "[SubsPlease] .. - 01 [1080p].mkv";
    let unsafe_dot = "[SubsPlease] . - 01 [1080p].mkv";
    let non_anime1 = "tutorial.mp4";
    let non_anime2 = "document[final].mp4";

    fs::write(env.source.join(ep1), b"ep1-bytes").unwrap();
    fs::write(env.source.join(ep2), b"ep2-bytes").unwrap();
    fs::write(env.source.join(ep3), b"ep3-new-bytes").unwrap();
    fs::write(env.source.join(ep4), b"ep4-bytes").unwrap();
    fs::write(env.source.join(unsafe_dotdot), b"dotdot-bytes").unwrap();
    fs::write(env.source.join(unsafe_dot), b"dot-bytes").unwrap();
    fs::write(env.source.join(non_anime1), b"tut").unwrap();
    fs::write(env.source.join(non_anime2), b"doc").unwrap();

    let dry_out = downsort::run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--dry-run",
    ])
    .expect("dry-run should succeed");
    assert!(
        dry_out.contains("Frieren - Beyond Journey's End"),
        "dry-run should mention show: {dry_out}"
    );
    assert!(!env.anime.exists(), "dry-run must not create anime directory");
    assert!(env.source.join(ep1).exists());

    let mob_dir = env.anime.join("Mob Psycho 100");
    fs::create_dir_all(&mob_dir).unwrap();
    fs::write(mob_dir.join(ep3), b"existing-ep3-bytes").unwrap();

    let live_out = downsort::run_cli(["--config", env.config_path.to_str().unwrap()])
        .expect("live run should succeed");

    assert!(env.anime.join("Frieren - Beyond Journey's End").join(ep1).exists());
    assert!(!env.source.join(ep1).exists());
    assert!(env.anime.join("Dandadan").join(ep2).exists());
    assert!(!env.source.join(ep2).exists());
    assert!(env.anime.join("Frieren").join(ep4).exists());
    assert!(!env.source.join(ep4).exists());

    assert!(env.source.join(unsafe_dotdot).exists(), "'..' show title must be ignored");
    assert!(!env.root.join(unsafe_dotdot).exists(), "'..' must not write outside anime dir");
    assert!(env.source.join(unsafe_dot).exists(), "'.' show title must be ignored");
    assert!(!env.anime.join(unsafe_dot).exists(), "'.' must not write directly into anime dir");

    assert_eq!(
        fs::read(mob_dir.join(ep3)).unwrap(),
        b"existing-ep3-bytes",
        "existing destination file must not be overwritten"
    );
    assert!(env.source.join(ep3).exists(), "colliding source file must remain in downloads");
    assert!(
        live_out.to_lowercase().contains("warn") || live_out.to_lowercase().contains("skip"),
        "live output should report collision warning/skip: {live_out}"
    );

    assert!(env.source.join(non_anime1).exists());
    assert!(env.source.join(non_anime2).exists());
    assert!(!env.anime.join("final").exists());
}

fn make_png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes
}

fn make_jpeg_header(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0xFF, 0xD8]);
    bytes.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00]);
    bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    bytes
}

fn make_webp_vp8x_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(b"WEBP");
    bytes.extend_from_slice(b"VP8X");
    bytes.extend_from_slice(&10u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    let w_minus_1 = width - 1;
    let h_minus_1 = height - 1;
    bytes.push((w_minus_1 & 0xFF) as u8);
    bytes.push(((w_minus_1 >> 8) & 0xFF) as u8);
    bytes.push(((w_minus_1 >> 16) & 0xFF) as u8);
    bytes.push((h_minus_1 & 0xFF) as u8);
    bytes.push(((h_minus_1 >> 8) & 0xFF) as u8);
    bytes.push(((h_minus_1 >> 16) & 0xFF) as u8);
    bytes
}

#[test]
fn sorts_wallpapers_by_aspect_ratio_and_resolution_and_handles_corrupt_and_collisions() {
    let env = TestEnv::new("wallpaper_sort");

    fs::write(env.source.join("land_1080p.png"), make_png_header(1920, 1080)).unwrap();
    fs::write(env.source.join("land_1440p.jpg"), make_jpeg_header(2560, 1440)).unwrap();
    fs::write(env.source.join("mob_2400.webp"), make_webp_vp8x_header(1080, 2400)).unwrap();
    fs::write(env.source.join("mob_2532.PNG"), make_png_header(1170, 2532)).unwrap();

    fs::write(env.source.join("square_1000.png"), make_png_header(1000, 1000)).unwrap();
    fs::write(env.source.join("boundary_1_2.png"), make_png_header(1536, 1280)).unwrap();
    fs::write(env.source.join("boundary_0_8.png"), make_png_header(1024, 1280)).unwrap();
    fs::write(env.source.join("lowres_800x600.jpg"), make_jpeg_header(800, 600)).unwrap();
    fs::write(env.source.join("corrupt.png"), b"not-a-valid-png").unwrap();
    fs::write(env.source.join("collide.png"), make_png_header(1920, 1080)).unwrap();

    let dry_out = downsort::run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--dry-run",
    ])
    .expect("dry-run should succeed");
    assert!(!env.desktop_wp.exists(), "dry-run must not create desktop_wp dir");
    assert!(!env.mobile_wp.exists(), "dry-run must not create mobile_wp dir");
    assert!(dry_out.contains("land_1080p.png"), "dry-run should preview wallpaper: {dry_out}");

    fs::create_dir_all(&env.desktop_wp).unwrap();
    fs::write(env.desktop_wp.join("collide.png"), b"existing-desktop-wallpaper").unwrap();

    let live_out = downsort::run_cli(["--config", env.config_path.to_str().unwrap()])
        .expect("live run should succeed despite corrupt image");

    assert!(env.desktop_wp.join("land_1080p.png").exists());
    assert!(env.desktop_wp.join("land_1440p.jpg").exists());
    assert!(!env.source.join("land_1080p.png").exists());
    assert!(!env.source.join("land_1440p.jpg").exists());

    assert!(env.mobile_wp.join("mob_2400.webp").exists());
    assert!(env.mobile_wp.join("mob_2532.PNG").exists());
    assert!(!env.source.join("mob_2400.webp").exists());
    assert!(!env.source.join("mob_2532.PNG").exists());

    assert!(env.source.join("square_1000.png").exists());
    assert!(env.source.join("boundary_1_2.png").exists());
    assert!(env.source.join("boundary_0_8.png").exists());
    assert!(env.source.join("lowres_800x600.jpg").exists());
    assert!(env.source.join("corrupt.png").exists());

    assert!(env.source.join("collide.png").exists());
    assert_eq!(
        fs::read(env.desktop_wp.join("collide.png")).unwrap(),
        b"existing-desktop-wallpaper"
    );
    assert!(live_out.to_lowercase().contains("warn"));
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

fn create_tar_gz(archive_path: &Path, base_dir: &Path, items: &[&str]) {
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(archive_path)
        .arg("-C")
        .arg(base_dir)
        .args(items)
        .status()
        .expect("tar command should run");
    assert!(status.success(), "tar creation failed");
}

#[test]
fn extracts_app_tarballs_creates_desktop_and_symlink_trashes_archive_and_skips_non_apps() {
    let env = TestEnv::new("app_tarball");
    let build_dir = env.root.join("build_tar");
    fs::create_dir_all(&build_dir).unwrap();

    let discord_stage = build_dir.join("Discord");
    fs::create_dir_all(&discord_stage).unwrap();
    let discord_bin = discord_stage.join("Discord");
    fs::write(&discord_bin, b"\x7fELF-discord-v1").unwrap();
    set_executable(&discord_bin);
    fs::write(discord_stage.join("discord.png"), make_png_header(256, 256)).unwrap();
    let discord_tar = env.source.join("discord-0.0.28.tar.gz");
    create_tar_gz(&discord_tar, &build_dir, &["Discord"]);

    let postman_stage = build_dir.join("postman_flat");
    fs::create_dir_all(&postman_stage).unwrap();
    let postman_sh = postman_stage.join("postman.sh");
    fs::write(&postman_sh, b"#!/usr/bin/env sh\necho postman\n").unwrap();
    set_executable(&postman_sh);
    let postman_tar = env.source.join("postman-linux-x64.tgz");
    create_tar_gz(&postman_tar, &postman_stage, &["postman.sh"]);

    let docs_stage = build_dir.join("docs_only");
    fs::create_dir_all(&docs_stage).unwrap();
    fs::write(docs_stage.join("README.md"), b"# Documentation\n").unwrap();
    fs::write(docs_stage.join("notes.txt"), b"plain text\n").unwrap();
    let docs_tar = env.source.join("project-docs-1.0.tar.gz");
    create_tar_gz(&docs_tar, &docs_stage, &["README.md", "notes.txt"]);

    let dry_out = env.run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--dry-run",
    ])
    .expect("dry-run should succeed");
    assert!(dry_out.contains("discord"), "dry-run should preview discord app: {dry_out}");
    assert!(dry_out.contains("Discord"), "dry-run should preview binary path: {dry_out}");
    assert!(dry_out.contains("discord.png"), "dry-run should preview icon path: {dry_out}");
    assert!(dry_out.contains("discord.desktop"), "dry-run should preview desktop path: {dry_out}");
    assert!(!env.apps.exists());
    assert!(!env.launchers.exists());
    assert!(!env.bin_dir.exists());
    assert!(discord_tar.exists());
    assert!(postman_tar.exists());
    assert!(docs_tar.exists());

    let live_out = env.run_cli(["--config", env.config_path.to_str().unwrap()])
        .expect("live run should succeed");
    assert!(live_out.contains("discord"));

    let installed_discord_bin = env.apps.join("discord").join("Discord");
    let installed_discord_icon = env.apps.join("discord").join("discord.png");
    assert!(installed_discord_bin.exists(), "Discord binary should be extracted");
    assert!(installed_discord_icon.exists(), "Discord icon should be extracted");

    let discord_desktop = env.launchers.join("discord.desktop");
    assert!(discord_desktop.exists(), "discord.desktop should be created");
    let desktop_text = fs::read_to_string(&discord_desktop).unwrap();
    assert!(desktop_text.contains("[Desktop Entry]"));
    assert!(desktop_text.contains("Type=Application"));
    assert!(desktop_text.contains("Name=Discord"));
    assert!(
        desktop_text.contains(&format!("Exec=\"{}\"", installed_discord_bin.display())),
        "Exec should be quoted in desktop file: {desktop_text}"
    );
    assert!(desktop_text.contains(&format!("Icon={}", installed_discord_icon.display())));
    assert!(desktop_text.contains("Categories=Utility;Application;"));

    let discord_link = env.bin_dir.join("discord");
    assert!(discord_link.exists(), "symlink in bin_dir should exist");
    assert_eq!(fs::read_link(&discord_link).unwrap(), installed_discord_bin);

    let postman_desktop = env.launchers.join("postman.desktop");
    assert!(postman_desktop.exists());
    let postman_text = fs::read_to_string(&postman_desktop).unwrap();
    assert!(postman_text.contains("Icon=application-x-executable"));
    assert!(env.bin_dir.join("postman").exists());

    assert!(!discord_tar.exists(), "discord archive should be moved out of downloads");
    assert!(!postman_tar.exists(), "postman archive should be moved out of downloads");
    let trash_files = env.xdg_data.join("Trash").join("files");
    assert!(trash_files.join("discord-0.0.28.tar.gz").exists(), "discord archive should be in XDG_DATA_HOME/Trash");
    assert!(trash_files.join("postman-linux-x64.tgz").exists(), "postman archive should be in XDG_DATA_HOME/Trash");
    assert!(!env.root.join("Trash").exists(), "Removed <launchers>/../Trash heuristic must not be used");

    assert!(docs_tar.exists(), "non-app tarball must remain in downloads");

    fs::write(&discord_bin, b"\x7fELF-discord-v2-upgraded").unwrap();
    set_executable(&discord_bin);
    let discord_tar_v2 = env.source.join("discord-0.0.29.tar.gz");
    create_tar_gz(&discord_tar_v2, &build_dir, &["Discord"]);

    env.run_cli(["--config", env.config_path.to_str().unwrap()])
        .expect("upgrade run should succeed");
    assert_eq!(
        fs::read(&installed_discord_bin).unwrap(),
        b"\x7fELF-discord-v2-upgraded",
        "existing app should be upgraded in place"
    );
    assert!(!discord_tar_v2.exists());

    // Verify relative Session Override paths with spaces produce absolute quoted Exec= and RFC 2396 percent-encoded .trashinfo Path=
    // and fallback to ~/.local/share/Trash when XDG_DATA_HOME is empty
    let space_source = env.root.join("My Downloads");
    fs::create_dir_all(&space_source).unwrap();
    let sublime_stage = build_dir.join("Sublime Text");
    fs::create_dir_all(&sublime_stage).unwrap();
    let sublime_bin = sublime_stage.join("sublime_text");
    fs::write(&sublime_bin, b"\x7fELF-sublime").unwrap();
    set_executable(&sublime_bin);
    let sublime_tar = space_source.join("sublime_text-4.0.tar.gz");
    create_tar_gz(&sublime_tar, &build_dir, &["Sublime Text"]);

    let exe = env!("CARGO_BIN_EXE_downsort");
    let rel_cmd = std::process::Command::new(exe)
        .current_dir(&env.root)
        .env("HOME", &env.root)
        .env("XDG_DATA_HOME", "")
        .args([
            "--config",
            "config.json",
            "--source",
            "./My Downloads",
            "--apps-dir",
            "./My Apps",
        ])
        .output()
        .expect("downsort should run with relative paths and spaces");
    assert!(rel_cmd.status.success(), "stderr: {}", String::from_utf8_lossy(&rel_cmd.stderr));

    let abs_sublime_bin = env.root.join("My Apps").join("sublime-text").join("sublime_text");
    assert!(abs_sublime_bin.exists());
    let sublime_desktop_text = fs::read_to_string(env.launchers.join("sublime-text.desktop")).unwrap();
    assert!(
        sublime_desktop_text.contains(&format!("Exec=\"{}\"", abs_sublime_bin.display())),
        "Desktop Exec should be quoted absolute path: {sublime_desktop_text}"
    );
    assert_eq!(
        fs::read_link(env.bin_dir.join("sublime-text")).unwrap(),
        abs_sublime_bin
    );

    let fallback_trash_info = env
        .root
        .join(".local/share/Trash/info/sublime_text-4.0.tar.gz.trashinfo");
    assert!(
        fallback_trash_info.exists(),
        "When XDG_DATA_HOME is empty, should use ~/.local/share/Trash"
    );
    let trashinfo_text = fs::read_to_string(&fallback_trash_info).unwrap();
    let expected_encoded_path = format!(
        "Path={}/My%20Downloads/sublime_text-4.0.tar.gz",
        env.root.display()
    );
    assert!(
        trashinfo_text.contains(&expected_encoded_path),
        "trashinfo Path= should be percent-encoded and absolute: {trashinfo_text}"
    );
}

#[test]
fn detects_elf_without_exec_bit_and_rejects_path_traversal_tarball() {
    let env = TestEnv::new("elf_and_traversal");
    let build_dir = env.root.join("build_tar");
    fs::create_dir_all(&build_dir).unwrap();

    let elf_stage = build_dir.join("neovim");
    fs::create_dir_all(&elf_stage).unwrap();
    let elf_bin = elf_stage.join("nvim");
    fs::write(&elf_bin, b"\x7fELF\x02\x01\x01\x00-neovim-binary").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&elf_bin).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&elf_bin, perms).unwrap();
    }
    let nvim_tar = env.source.join("nvim-linux64.tar.gz");
    create_tar_gz(&nvim_tar, &build_dir, &["neovim"]);

    let evil_dir = build_dir.join("evil_sub");
    fs::create_dir_all(&evil_dir).unwrap();
    let outside_evil = build_dir.join("escaped.sh");
    fs::write(&outside_evil, b"#!/bin/sh\necho pwned\n").unwrap();
    set_executable(&outside_evil);
    let evil_tar = env.source.join("evil-1.0.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-P")
        .arg("-czf")
        .arg(&evil_tar)
        .arg("-C")
        .arg(&evil_dir)
        .arg("../escaped.sh")
        .status()
        .expect("tar should build traversal archive");
    assert!(status.success());

    // Archive with symlink pointing to /etc/passwd
    let sym_abs_stage = build_dir.join("sym_abs");
    fs::create_dir_all(&sym_abs_stage).unwrap();
    let sym_abs_bin = sym_abs_stage.join("symapp");
    fs::write(&sym_abs_bin, b"\x7fELF-symapp").unwrap();
    set_executable(&sym_abs_bin);
    std::os::unix::fs::symlink("/etc/passwd", sym_abs_stage.join("passwd_link")).unwrap();
    let sym_abs_tar = env.source.join("symapp-1.0.tar.gz");
    create_tar_gz(&sym_abs_tar, &build_dir, &["sym_abs"]);

    // Archive with symlink pointing to ../../outside
    let sym_rel_stage = build_dir.join("sym_rel");
    fs::create_dir_all(&sym_rel_stage).unwrap();
    let sym_rel_bin = sym_rel_stage.join("relapp");
    fs::write(&sym_rel_bin, b"\x7fELF-relapp").unwrap();
    set_executable(&sym_rel_bin);
    std::os::unix::fs::symlink("../../outside", sym_rel_stage.join("escape_link")).unwrap();
    let sym_rel_tar = env.source.join("relapp-1.0.tar.gz");
    create_tar_gz(&sym_rel_tar, &build_dir, &["sym_rel"]);

    // Archive with hardlink pointing to /etc/passwd
    let hard_tar = env.source.join("hardapp-1.0.tar.gz");
    let py_hardlink = r#"
import io, sys, tarfile
with tarfile.open(sys.argv[1], "w:gz") as tf:
    data = b"\x7fELF-hardapp"
    ti_bin = tarfile.TarInfo("hardapp/hardapp")
    ti_bin.size = len(data)
    ti_bin.mode = 0o755
    tf.addfile(ti_bin, io.BytesIO(data))
    ti_lnk = tarfile.TarInfo("hardapp/evil_hardlink")
    ti_lnk.type = tarfile.LNKTYPE
    ti_lnk.linkname = "/etc/passwd"
    tf.addfile(ti_lnk)
"#;
    let hard_status = std::process::Command::new("python3")
        .arg("-c")
        .arg(py_hardlink)
        .arg(&hard_tar)
        .status()
        .expect("python3 should create hardlink tar.gz");
    assert!(hard_status.success());

    // Archive with top-level non-executable .sh script (mode 0644, depth 1 after root strip) -> should qualify
    let top_sh_stage = build_dir.join("topsh");
    fs::create_dir_all(&top_sh_stage).unwrap();
    let top_sh_file = top_sh_stage.join("topsh.sh");
    fs::write(&top_sh_file, b"#!/bin/sh\necho topsh\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&top_sh_file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&top_sh_file, perms).unwrap();
    }
    let top_sh_tar = env.source.join("topsh-1.0.tar.gz");
    create_tar_gz(&top_sh_tar, &build_dir, &["topsh"]);

    // Archive with only a nested non-executable .sh script (mode 0644)
    let nested_sh_stage = build_dir.join("libpkg").join("subdir");
    fs::create_dir_all(&nested_sh_stage).unwrap();
    let helper_sh = nested_sh_stage.join("helper.sh");
    fs::write(&helper_sh, b"#!/bin/sh\necho helper\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&helper_sh).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&helper_sh, perms).unwrap();
    }
    let nested_sh_tar = env.source.join("libpkg-1.0.tar.gz");
    create_tar_gz(&nested_sh_tar, &build_dir, &["libpkg"]);

    // Archive with top-level directory AND top-level symlink: dry-run and live must agree
    let multi_top_stage = build_dir.join("multi_top");
    let multi_sub = multi_top_stage.join("runner");
    fs::create_dir_all(&multi_sub).unwrap();
    let runner_bin = multi_sub.join("runner");
    fs::write(&runner_bin, b"\x7fELF-runner").unwrap();
    set_executable(&runner_bin);
    fs::write(multi_sub.join("runner.png"), make_png_header(64, 64)).unwrap();
    std::os::unix::fs::symlink("runner/runner", multi_top_stage.join("runner-link")).unwrap();
    let runner_tar = env.source.join("runner-1.0.tar.gz");
    create_tar_gz(&runner_tar, &multi_top_stage, &["runner", "runner-link"]);

    let exe = env!("CARGO_BIN_EXE_downsort");
    let dry_run_cmd = std::process::Command::new(exe)
        .env("HOME", &env.root)
        .env("XDG_DATA_HOME", &env.xdg_data)
        .args(["--config", env.config_path.to_str().unwrap(), "--dry-run"])
        .output()
        .expect("dry-run binary should run");
    assert!(dry_run_cmd.status.success());
    let dry_out = String::from_utf8_lossy(&dry_run_cmd.stdout);

    let live_cmd = std::process::Command::new(exe)
        .env("HOME", &env.root)
        .env("XDG_DATA_HOME", &env.xdg_data)
        .args(["--config", env.config_path.to_str().unwrap()])
        .output()
        .expect("live binary should run");
    assert!(live_cmd.status.success());
    let out = String::from_utf8_lossy(&live_cmd.stdout);

    let expected_runner_bin = env.apps.join("runner").join("runner").join("runner");
    let expected_runner_icon = env.apps.join("runner").join("runner").join("runner.png");
    assert!(
        dry_out.contains(&format!("bin={}", expected_runner_bin.display())),
        "dry-run should predict unstripped path when top-level symlink exists: {dry_out}"
    );
    assert!(
        dry_out.contains(&format!("icon={}", expected_runner_icon.display())),
        "dry-run should predict unstripped icon path: {dry_out}"
    );
    assert!(
        out.contains(&format!("bin={}", expected_runner_bin.display())),
        "live run should match dry-run binary path: {out}"
    );
    assert!(expected_runner_bin.exists());

    let installed_nvim = env.apps.join("nvim").join("nvim");
    assert!(
        installed_nvim.exists(),
        "ELF binary without +x bit in tar should still be detected and installed: {out}"
    );
    assert!(env.launchers.join("nvim.desktop").exists());
    assert!(env.bin_dir.join("nvim").exists());

    assert!(evil_tar.exists(), "rejected traversal archive should remain in downloads");
    assert!(!env.apps.join("evil").exists());
    assert!(!env.root.join("escaped.sh").exists());
    assert!(sym_abs_tar.exists(), "symlink traversal to /etc/passwd must be rejected");
    assert!(!env.apps.join("symapp").exists());
    assert!(sym_rel_tar.exists(), "symlink traversal to ../../outside must be rejected");
    assert!(!env.apps.join("relapp").exists());
    assert!(hard_tar.exists(), "hardlink traversal to /etc/passwd must be rejected");
    assert!(!env.apps.join("hardapp").exists());
    assert!(
        env.apps.join("topsh").join("topsh.sh").exists(),
        "top-level non-executable .sh should qualify and be installed"
    );
    assert!(nested_sh_tar.exists(), "archive with only nested non-exec .sh must be ignored");
    assert!(!env.apps.join("libpkg").exists());
    assert!(out.to_lowercase().contains("warn"));
}

#[test]
fn end_to_end_mixed_batch_via_cli_binary_dry_run_and_live() {
    let env = TestEnv::new("e2e_pipeline");
    let build_dir = env.root.join("build_tar");
    fs::create_dir_all(&build_dir).unwrap();

    let anime_file = "[SubsPlease] Solo Leveling - 05 [1080p].mkv";
    fs::write(env.source.join(anime_file), b"anime-data").unwrap();
    fs::write(env.source.join("home_video.mp4"), b"personal-video").unwrap();
    fs::write(env.source.join("desktop_bg.png"), make_png_header(3840, 2160)).unwrap();
    fs::write(env.source.join("phone_bg.jpg"), make_jpeg_header(1080, 2400)).unwrap();
    fs::write(env.source.join("avatar.png"), make_png_header(512, 512)).unwrap();
    fs::write(env.source.join("broken_img.webp"), b"corrupt-webp-bytes").unwrap();
    fs::write(env.source.join("unfinished.mkv.crdownload"), b"partial").unwrap();
    fs::write(env.source.join("broken_archive.tar.gz"), b"not-a-valid-gzip").unwrap();

    let app_stage = build_dir.join("wezterm");
    fs::create_dir_all(&app_stage).unwrap();
    let app_bin = app_stage.join("wezterm");
    fs::write(&app_bin, b"\x7fELF-wezterm").unwrap();
    set_executable(&app_bin);
    create_tar_gz(&env.source.join("wezterm-2026.tar.gz"), &build_dir, &["wezterm"]);

    let src_stage = build_dir.join("src_code");
    fs::create_dir_all(&src_stage).unwrap();
    fs::write(src_stage.join("main.c"), b"int main() { return 0; }\n").unwrap();
    create_tar_gz(&env.source.join("source-code-1.0.tar.gz"), &src_stage, &["main.c"]);

    let snapshot_before = snapshot_dir(&env.source);

    let exe = env!("CARGO_BIN_EXE_downsort");
    let dry_cmd = std::process::Command::new(exe)
        .env("HOME", &env.root)
        .env("XDG_DATA_HOME", &env.xdg_data)
        .args(["--config", env.config_path.to_str().unwrap(), "--dry-run"])
        .output()
        .expect("downsort binary should run");
    assert!(dry_cmd.status.success(), "dry-run should exit 0");

    let snapshot_after_dry = snapshot_dir(&env.source);
    assert_eq!(snapshot_before, snapshot_after_dry, "dry-run must not modify any files");
    assert!(!env.anime.exists());
    assert!(!env.desktop_wp.exists());
    assert!(!env.mobile_wp.exists());
    assert!(!env.apps.exists());

    let live_cmd = std::process::Command::new(exe)
        .env("HOME", &env.root)
        .env("XDG_DATA_HOME", &env.xdg_data)
        .args(["--config", env.config_path.to_str().unwrap()])
        .output()
        .expect("downsort binary should run");
    assert!(live_cmd.status.success(), "live run should exit 0 even with per-file errors");
    let stdout = String::from_utf8_lossy(&live_cmd.stdout);

    assert!(stdout.contains("Total files scanned: 10"), "stdout: {stdout}");
    assert!(stdout.contains("Anime videos moved: 1"), "stdout: {stdout}");
    assert!(stdout.contains("Solo Leveling: 1"), "stdout: {stdout}");
    assert!(
        stdout.contains("Wallpapers organized: 2 (Desktop: 1, Mobile: 1)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Applications installed / updated: 1"), "stdout: {stdout}");
    assert!(stdout.contains("Files skipped: 6"), "stdout: {stdout}");

    assert!(env.anime.join("Solo Leveling").join(anime_file).exists());
    assert!(env.desktop_wp.join("desktop_bg.png").exists());
    assert!(env.mobile_wp.join("phone_bg.jpg").exists());
    assert!(env.apps.join("wezterm").join("wezterm").exists());
    assert!(env.launchers.join("wezterm.desktop").exists());
    assert!(env.bin_dir.join("wezterm").exists());
    assert!(env.xdg_data.join("Trash").join("files").join("wezterm-2026.tar.gz").exists());

    assert!(env.source.join("home_video.mp4").exists());
    assert!(env.source.join("avatar.png").exists());
    assert!(env.source.join("broken_img.webp").exists());
    assert!(env.source.join("unfinished.mkv.crdownload").exists());
    assert!(env.source.join("broken_archive.tar.gz").exists());
    assert!(env.source.join("source-code-1.0.tar.gz").exists());
}

#[test]
fn session_overrides_save_config_and_structured_json_output() {
    let env = TestEnv::new("json_and_overrides");
    let custom_anime = env.root.join("custom_anime");
    let custom_desktop = env.root.join("custom_desktop");
    let custom_mobile = env.root.join("custom_mobile");
    let custom_apps = env.root.join("custom_apps");

    let ep = "[SubsPlease] Dandadan - 03 [1080p].mkv";
    fs::write(env.source.join(ep), b"anime").unwrap();
    fs::write(env.source.join("wall.png"), make_png_header(1920, 1080)).unwrap();
    fs::write(env.source.join("square.png"), make_png_header(600, 600)).unwrap();
    fs::write(env.source.join("partial.crdownload"), b"part").unwrap();
    fs::write(env.source.join("notes.txt"), b"txt").unwrap();

    // 1. Dry-run with --json and Session Overrides: must not mutate config.json on disk
    let orig_config = fs::read_to_string(&env.config_path).unwrap();
    let dry_json = downsort::run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--anime-dir",
        custom_anime.to_str().unwrap(),
        "--desktop-wallpapers-dir",
        custom_desktop.to_str().unwrap(),
        "--mobile-wallpapers-dir",
        custom_mobile.to_str().unwrap(),
        "--apps-dir",
        custom_apps.to_str().unwrap(),
        "--dry-run",
        "--json",
    ])
    .expect("dry-run --json should succeed");

    assert_eq!(
        fs::read_to_string(&env.config_path).unwrap(),
        orig_config,
        "Session Overrides must not overwrite saved config.json"
    );
    assert!(dry_json.contains("\"dry_run\": true"), "json: {dry_json}");
    assert!(dry_json.contains("\"total_scanned\": 5"), "json: {dry_json}");
    assert!(dry_json.contains("\"anime_moved\": 1"), "json: {dry_json}");
    assert!(dry_json.contains("\"desktop_wallpapers_moved\": 1"), "json: {dry_json}");
    assert!(dry_json.contains("\"files_skipped\": 3"), "json: {dry_json}");
    assert!(dry_json.contains("\"status\": \"Will Move\""), "json: {dry_json}");
    assert!(dry_json.contains("\"reason\": \"Active Download\""), "json: {dry_json}");
    assert!(dry_json.contains("\"reason\": \"Square / Low-Res\""), "json: {dry_json}");
    assert!(
        dry_json.contains(&custom_anime.display().to_string()),
        "destination should use Session Override anime dir: {dry_json}"
    );

    // 2. Live run with Session Overrides and --json
    let live_json = downsort::run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--anime-dir",
        custom_anime.to_str().unwrap(),
        "--desktop-wallpapers-dir",
        custom_desktop.to_str().unwrap(),
        "--json",
    ])
    .expect("live --json should succeed");

    assert!(live_json.contains("\"dry_run\": false"), "json: {live_json}");
    assert!(live_json.contains("\"status\": \"Moved\""), "json: {live_json}");
    assert!(custom_anime.join("Dandadan").join(ep).exists());
    assert!(custom_desktop.join("wall.png").exists());
    assert_eq!(
        fs::read_to_string(&env.config_path).unwrap(),
        orig_config,
        "Live run with Session Overrides still must not overwrite config.json without --save-config"
    );

    // 3. --save-config persists the Session Overrides to config.json
    downsort::run_cli([
        "--config",
        env.config_path.to_str().unwrap(),
        "--anime-dir",
        custom_anime.to_str().unwrap(),
        "--desktop-wallpapers-dir",
        custom_desktop.to_str().unwrap(),
        "--mobile-wallpapers-dir",
        custom_mobile.to_str().unwrap(),
        "--apps-dir",
        custom_apps.to_str().unwrap(),
        "--save-config",
    ])
    .expect("--save-config should succeed");

    let updated_config = fs::read_to_string(&env.config_path).unwrap();
    assert!(updated_config.contains(&custom_anime.display().to_string()));
    assert!(updated_config.contains(&custom_desktop.display().to_string()));
    assert!(updated_config.contains(&custom_mobile.display().to_string()));
    assert!(updated_config.contains(&custom_apps.display().to_string()));
}

#[test]
fn pyqt6_gui_loads_directories_previews_all_files_saves_config_and_sorts_live() {
    let env = TestEnv::new("pyqt6_gui");
    let override_anime = env.root.join("gui_override_anime");
    let override_desktop = env.root.join("gui_override_desktop");

    let ep = "[SubsPlease] Bleach - 01 [1080p].mkv";
    fs::write(env.source.join(ep), b"bleach-video").unwrap();
    fs::write(env.source.join("wall.png"), make_png_header(1920, 1080)).unwrap();
    fs::write(env.source.join("readme.txt"), b"hello").unwrap();

    let exe = env!("CARGO_BIN_EXE_downsort");
    let orig_config = fs::read_to_string(&env.config_path).unwrap();

    // Drive the PyQt6 MainWindow in offscreen mode through Python
    let py_driver = r#"
import os, sys
os.environ["QT_QPA_PLATFORM"] = "offscreen"
from PyQt6.QtWidgets import QApplication
import gui

app = QApplication([])
cli_bin = sys.argv[1]
cfg_path = sys.argv[2]
override_anime = sys.argv[3]
override_desktop = sys.argv[4]
orig_cfg_text = open(cfg_path).read()

win = gui.DownsortWindow(cli_bin=cli_bin, config_path=cfg_path)

# 1. Verify initial 5 directory inputs loaded from config.json and initial Dry Run Preview populated all 3 files
assert win.source_input.text().endswith("downloads"), win.source_input.text()
assert win.anime_input.text().endswith("anime"), win.anime_input.text()
assert win.desktop_wp_input.text().endswith("desktop_wp"), win.desktop_wp_input.text()
assert win.mobile_wp_input.text().endswith("mobile_wp"), win.mobile_wp_input.text()
assert win.apps_input.text().endswith("apps"), win.apps_input.text()

assert win.table.columnCount() == 4
assert win.table.rowCount() == 3, f"Expected 3 rows in preview table, got {win.table.rowCount()}"

# 2. Edit anime & desktop wallpaper inputs (Session Override) and click Preview
win.anime_input.setText(override_anime)
win.desktop_wp_input.setText(override_desktop)
win.preview_btn.click()

# Verify config.json on disk is STILL untouched after Session Override preview
assert open(cfg_path).read() == orig_cfg_text, "Preview must not overwrite config.json"

# Verify table destinations reflect the Session Override paths
destinations = [win.table.item(r, 3).text() for r in range(win.table.rowCount())]
assert any(override_anime in d for d in destinations), f"Expected {override_anime} in {destinations}"
assert any(override_desktop in d for d in destinations), f"Expected {override_desktop} in {destinations}"

# 3. Click Sort Now: files should be moved using Session Override paths, config.json still untouched
win.sort_btn.click()
assert open(cfg_path).read() == orig_cfg_text, "Sort Now must not overwrite config.json without Save Config"
assert "Anime: 1" in win.summary_label.text() or "Moved" in win.summary_label.text(), win.summary_label.text()

# 4. Click Save Config: config.json on disk should now persist the new paths
win.save_config_btn.click()
saved_cfg = open(cfg_path).read()
assert override_anime in saved_cfg, saved_cfg
assert override_desktop in saved_cfg, saved_cfg
print("GUI_TEST_OK")
"#;

    let cmd = std::process::Command::new("python3")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("PYTHONPATH", concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .arg("-c")
        .arg(py_driver)
        .arg(exe)
        .arg(&env.config_path)
        .arg(&override_anime)
        .arg(&override_desktop)
        .output()
        .expect("python3 should run PyQt6 test");

    let stdout = String::from_utf8_lossy(&cmd.stdout);
    let stderr = String::from_utf8_lossy(&cmd.stderr);
    assert!(
        cmd.status.success() && stdout.contains("GUI_TEST_OK"),
        "PyQt6 GUI test failed.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Confirm files actually moved to Session Override directories on disk
    assert!(override_anime.join("Bleach").join(ep).exists());
    assert!(override_desktop.join("wall.png").exists());
    assert!(env.source.join("readme.txt").exists());
    assert_ne!(fs::read_to_string(&env.config_path).unwrap(), orig_config);
}
