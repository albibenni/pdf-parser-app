.PHONY: i dev build build-aur install-aur test test-ui test-rust lint format clean

i:
	pnpm install

# Uses a desktop-only config and port 1422, matching test-yourself. This lets
# another Tauri app keep using the conventional port 1420 during development.
dev:
	PORT=1422 pnpm tauri dev --config src-tauri/tauri.desktop.conf.json

build:
	pnpm tauri build

# Builds a local Arch package from the AUR recipe in aur/.
build-aur:
	cd aur && makepkg -s

install-aur:
	cd aur && makepkg -si

test: test-ui test-rust

test-ui:
	pnpm test

test-rust:
	cd src-tauri && cargo test

lint:
	pnpm lint

format:
	pnpm indent:write
	cd src-tauri && cargo fmt

clean:
	rm -rf node_modules dist coverage src-tauri/target
