# PDF Parser

A local-first Tauri desktop app that converts PDFs to Markdown using
[Marker](https://github.com/datalab-to/marker).

## Current state

The app provides the Tauri UI, file/folder selection, conversion modes, queue,
and a private Marker runtime. On first setup, the app creates its own Python
environment under its app-data folder and installs `marker-pdf` there. It does
not use or modify a system-wide Marker installation, and it does not send PDFs
to a hosted service. OCR conversion also needs `llama-server`: setup checks for
it and installs `llama.cpp` with Homebrew on macOS or authenticated `pacman` on
Arch Linux.

The initial model download happens when Marker first processes an applicable
document.

## Development

```bash
pnpm install
pnpm tauri dev
```

Checks:

```bash
pnpm lint
pnpm test
pnpm tauri build --no-bundle
```

### Arch Linux package

Build the package locally with `make build-aur`. It uses
[aur/PKGBUILD](aur/PKGBUILD) and writes the resulting package archive into
`aur/`. Run `make install-aur` to build and install it locally.

## Modes

- **Fast**: default for M-series Macs and CPU-only Linux.
- **Balanced**: additional OCR/layout work for difficult PDFs.
- **Text only**: passes `--disable_ocr`; ideal for clean, born-digital PDFs.
