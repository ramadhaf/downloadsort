import argparse
from dataclasses import dataclass
import json
import subprocess
import sys
from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import (
    QApplication,
    QFileDialog,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QMainWindow,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)


@dataclass(frozen=True)
class DirectorySpec:
    attr_name: str
    config_key: str
    cli_flag: str
    label: str
    default_path: str


DIRECTORY_SPECS: tuple[DirectorySpec, ...] = (
    DirectorySpec("source_input", "source", "--source", "Source Directory:", "~/Downloads"),
    DirectorySpec("anime_input", "anime", "--anime-dir", "Anime Directory:", "~/Videos/Anime"),
    DirectorySpec(
        "desktop_wp_input",
        "desktop_wallpapers",
        "--desktop-wallpapers-dir",
        "Desktop Wallpapers Directory:",
        "~/Pictures/Wallpapers/Desktop",
    ),
    DirectorySpec(
        "mobile_wp_input",
        "mobile_wallpapers",
        "--mobile-wallpapers-dir",
        "Mobile Wallpapers Directory:",
        "~/Pictures/Wallpapers/Mobile",
    ),
    DirectorySpec(
        "apps_input",
        "applications",
        "--apps-dir",
        "Applications Directory:",
        "~/Applications",
    ),
)


class DownsortWindow(QMainWindow):
    def __init__(
        self,
        cli_bin: str,
        config_path: str | None = None,
        initial_overrides: dict[str, str] | None = None,
    ):
        super().__init__()
        self.cli_bin = cli_bin
        self.config_path = config_path
        self.setWindowTitle("Downsort — Download File Organizer")
        self.resize(980, 640)

        central = QWidget(self)
        self.setCentralWidget(central)
        root_layout = QVBoxLayout(central)
        root_layout.setContentsMargins(14, 14, 14, 14)
        root_layout.setSpacing(12)

        dirs_group = QGroupBox("Directories (Session Overrides until Save Config is clicked)")
        grid = QGridLayout(dirs_group)
        grid.setColumnStretch(1, 1)

        self._dir_inputs: dict[str, QLineEdit] = {}
        for row_idx, spec in enumerate(DIRECTORY_SPECS):
            line_edit, _browse_btn = self._add_dir_row(grid, row_idx, spec.label)
            setattr(self, spec.attr_name, line_edit)
            self._dir_inputs[spec.config_key] = line_edit

        root_layout.addWidget(dirs_group)

        self.table = QTableWidget(0, 4, self)
        self.table.setHorizontalHeaderLabels(["File", "Category", "Status / Reason", "Destination"])
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        self.table.setSelectionBehavior(QTableWidget.SelectionBehavior.SelectRows)
        self.table.setAlternatingRowColors(True)
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeMode.Stretch)
        root_layout.addWidget(self.table, stretch=1)

        bottom_bar = QHBoxLayout()
        self.summary_label = QLabel("Ready")
        bottom_bar.addWidget(self.summary_label, stretch=1)

        self.preview_btn = QPushButton("Preview / Refresh")
        self.preview_btn.clicked.connect(self.run_preview)
        bottom_bar.addWidget(self.preview_btn)

        self.save_config_btn = QPushButton("Save Config")
        self.save_config_btn.clicked.connect(self.save_config)
        bottom_bar.addWidget(self.save_config_btn)

        self.sort_btn = QPushButton("Sort Now")
        self.sort_btn.clicked.connect(self.run_sort)
        bottom_bar.addWidget(self.sort_btn)

        root_layout.addLayout(bottom_bar)

        self.load_config_into_inputs(initial_overrides or {})
        self.run_preview()

    def _add_dir_row(self, grid: QGridLayout, row: int, label_text: str) -> tuple[QLineEdit, QPushButton]:
        label = QLabel(label_text)
        line_edit = QLineEdit()
        browse_btn = QPushButton("Browse...")
        browse_btn.clicked.connect(
            lambda _checked=False, le=line_edit, title=label_text: self._browse_dir(le, title)
        )
        grid.addWidget(label, row, 0)
        grid.addWidget(line_edit, row, 1)
        grid.addWidget(browse_btn, row, 2)
        return line_edit, browse_btn

    def _browse_dir(self, line_edit: QLineEdit, title: str) -> None:
        selected = QFileDialog.getExistingDirectory(self, f"Select {title.rstrip(':')}", line_edit.text())
        if selected:
            line_edit.setText(selected)
            self.run_preview()

    def _base_cli_args(self) -> list[str]:
        args = [self.cli_bin]
        if self.config_path:
            args.extend(["--config", self.config_path])
        return args

    def _session_override_args(self) -> list[str]:
        args = self._base_cli_args()
        for spec in DIRECTORY_SPECS:
            value = self._dir_inputs[spec.config_key].text().strip()
            if value:
                args.extend([spec.cli_flag, value])
        return args

    def load_config_into_inputs(self, overrides: dict[str, str]) -> None:
        proc = subprocess.run(
            self._base_cli_args() + ["--show-config"],
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            self.summary_label.setText(f"Config error: {proc.stderr.strip()}")
            return

        cfg = json.loads(proc.stdout)
        for spec in DIRECTORY_SPECS:
            self._dir_inputs[spec.config_key].setText(
                overrides.get(spec.config_key) or cfg.get(spec.config_key, spec.default_path)
            )

    def run_preview(self) -> None:
        self._execute_scan(dry_run=True)

    def run_sort(self) -> None:
        self._execute_scan(dry_run=False)

    def save_config(self) -> None:
        proc = subprocess.run(
            self._session_override_args() + ["--save-config"],
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode == 0:
            self.summary_label.setText("Configuration saved to config.json")
        else:
            self.summary_label.setText(f"Failed to save config: {proc.stderr.strip()}")

    def _execute_scan(self, dry_run: bool) -> None:
        cmd = self._session_override_args() + ["--json"]
        if dry_run:
            cmd.append("--dry-run")

        proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if proc.returncode != 0:
            self.table.setRowCount(0)
            self.summary_label.setText(f"Error: {proc.stderr.strip()}")
            return

        data = json.loads(proc.stdout)
        items = data.get("items", [])
        summary = data.get("summary", {})

        self.table.setRowCount(len(items))
        for row_idx, item in enumerate(items):
            values = [
                item.get("file", ""),
                item.get("category", ""),
                f"{item.get('status', '')} ({item.get('reason', '')})"
                if item.get("reason")
                else item.get("status", ""),
                item.get("destination", "") or "—",
            ]
            for col_idx, text in enumerate(values):
                cell = QTableWidgetItem(text)
                cell.setFlags(cell.flags() & ~Qt.ItemFlag.ItemIsEditable)
                self.table.setItem(row_idx, col_idx, cell)

        mode_prefix = "Dry Run Preview" if dry_run else "Sorted Live"
        self.summary_label.setText(
            f"{mode_prefix} — Scanned: {summary.get('total_scanned', 0)} | "
            f"Anime: {summary.get('anime_moved', 0)} | "
            f"Desktop WP: {summary.get('desktop_wallpapers_moved', 0)} | "
            f"Mobile WP: {summary.get('mobile_wallpapers_moved', 0)} | "
            f"Apps: {summary.get('apps_installed', 0)} | "
            f"Skipped/Ignored: {summary.get('files_skipped', 0)}"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Downsort PyQt6 GUI")
    parser.add_argument("--cli-bin", required=True)
    parser.add_argument("--config", default=None)
    for spec in DIRECTORY_SPECS:
        parser.add_argument(spec.cli_flag, dest=spec.config_key, default=None)
    args = parser.parse_args(argv)

    overrides = {
        spec.config_key: getattr(args, spec.config_key)
        for spec in DIRECTORY_SPECS
        if getattr(args, spec.config_key) is not None
    }

    app = QApplication(sys.argv[:1])
    win = DownsortWindow(cli_bin=args.cli_bin, config_path=args.config, initial_overrides=overrides)
    win.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
