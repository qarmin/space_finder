# Space Finder

A fast, visual disk-space analyser with an interactive treemap.
Scan any folder (or multiple paths), see which files and directories consume the most space, and drill down interactively.

<div align="center"><img src="https://github.com/user-attachments/assets/f40a8dac-741d-4b39-bc45-3d8ac8756c47" alt="img" /></div>

## Features

- **Treemap** – proportional rectangles; larger rect = more disk space used
- **Folder composition** – directory blocks show an internal colour gradient (left → right) representing the file-type breakdown, with diagonal hatching so they are visually distinct from files
- **Interactive zoom** – scroll wheel zooms into / out of any selected path
- **Right-click context menu** – open item or its parent in the system file manager
- **Top-files list** – quick list of the 32 largest files found
- **Multilingual** – automatically uses the OS locale; English and Polish translations included

## Supported platforms

| Platform               | Tested |
|------------------------|--------|
| Linux (x86-64)         | ✅      |
| Windows (x86-64)       | ✅      |
| macOS (arm64 / x86-64) | ✅      |

## Download

Pre-built binaries are attached to every CI run as **GitHub Actions artifacts** (see the *Actions* tab).

## Building from source

```bash
# Prerequisites: Rust stable toolchain

cargo build --release
# binary: target/release/space_finder
```

## Usage

1. Click **Show** next to *Scan Sources* to add folders or files.
2. Click **Start** – the chart renders automatically when the scan finishes.
3. **Left-click** a block to select it; **scroll up** to zoom in toward the selection, **scroll down** to zoom out.
4. **Right-click** to open the item or its parent folder in the file manager.

## Translations

Translation files live in `i18n/<lang>/space_finder.ftl` (Mozilla Fluent format).
The app picks the OS locale automatically; English is the fallback.

To add a new language:
1. Create `i18n/<lang-code>/space_finder.ftl`
2. Copy `i18n/en/space_finder.ftl` and translate the values.

## License

MIT

---

> **Note:** this project was developed with AI assistance (GitHub Copilot / Claude).
