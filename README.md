# PDF Parser

A local-first Tauri desktop app that converts PDFs to Markdown using
[Marker](https://github.com/datalab-to/marker).

## Current state

The app provides the Tauri UI, file/folder selection, conversion modes, queue,
and a Rust command that calls the local `marker_single` executable. It does not
send PDFs to a hosted service.

For now, install Marker independently in the user environment that launches
the app. The next packaging step is a managed runtime installer that downloads
Marker, its models, and `llama.cpp` into the app data directory on first use.

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
