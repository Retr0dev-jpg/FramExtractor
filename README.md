# FramExtractor

Desktop app that extracts **every frame** from one or more videos and saves them as PNG files.

On first launch it automatically downloads `ffmpeg` into a temp folder (no separate install needed). Later launches reuse that copy.

## Usage

1. Start `FramExtractor`.
2. Add videos in any of these ways:
   - drag and drop files onto the window
   - **Add…** button
   - **Ctrl+V** (file paths)
3. Optional: 📌 pins the window on top.

Each video is processed in parallel, with a progress bar. When extraction finishes, the row disappears on its own.

## Output

A folder is created next to the video, named after the file (e.g. `clip.mp4` → `clip/`). If that folder already exists, Windows-style suffixes are used: `clip (1)`, `clip (2)`, …

Frames are named `frame_000001.png`, `frame_000002.png`, …

If extraction fails, that folder is deleted.

## Formats

`mp4` `mkv` `avi` `mov` `wmv` `flv` `webm` `m4v` `ts` `3gp`

## Requirements and build

- Rust (edition 2021)
- Windows recommended (the UI is aimed at the Windows desktop)

```bash
cargo run --release
```

The executable is `target/release/FramExtractor.exe`.
