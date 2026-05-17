# Space Finder

A fast, visual disk-space analyser with an interactive treemap.
Scan any folder (or multiple paths), see which files and directories consume the most space, and drill down interactively.

<div align="center"><img src="https://github.com/user-attachments/assets/f40a8dac-741d-4b39-bc45-3d8ac8756c47" alt="img" /></div>

## Features

- **Treemap** – proportional rectangles; larger rect = more disk space used
- **Folder composition** – directory blocks show an internal colour gradient (left → right) representing the file-type breakdown, with diagonal hatching so they are visually distinct from files
- **Interactive zoom** – scroll wheel zooms into / out of any selected path
- **Click to inspect** – left-click a block to see its name, path and size
- **Category filter** – toggle file categories (audio, video, images, archives, documents, code, …) on the chart
- **Right-click context menu** – open the item or its parent in the system file manager
- **Top-files list** – quick list of the 32 largest files found
- **Dark mode** – togglable light / dark theme, persisted between runs
- **Persisted sources** – the last scanned path list is restored on startup

## Download

Pre-built binaries are available on the [releases page](https://github.com/qarmin/space_finder/releases).

Alternatively, build from source (requires the Rust stable toolchain):

```bash
cargo build --release
# binary: target/release/space_finder
```

## Usage

1. Click **Show** next to *Scan Sources* to add folders or files.
2. Click **Start** – the chart renders automatically when the scan finishes.
3. **Left-click** a block to select it; **scroll up** to zoom in toward the selection, **scroll down** to zoom out.
4. **Right-click** to open the item or its parent folder in the file manager.

## License

GPL-3.0 - project as a whole is licensed under this license due to Slint licensing requirements.

MIT - the source code itself is licensed under this license.


---

> **Note:** this project was developed with AI assistance (GitHub Copilot / Claude).
