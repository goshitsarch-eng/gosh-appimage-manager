# Recipes for Gosh AppImage Manager 3.0.0 (Rust + libcosmic).
# Run `just <recipe>`. The GUI needs network once for the libcosmic checkout.

# Debug CLI build (no GUI deps)
build:
    cargo build

# Debug build with the libcosmic GUI
build-gui:
    cargo build --features gui

# Release build with the libcosmic GUI (what Flatpak ships)
release:
    cargo build --release --features gui

# Full test suite (fake seams + synthetic fixtures; no network, no home writes)
test:
    cargo test

lint:
    cargo clippy --all-targets -- -D warnings

fmt-check:
    cargo fmt --check

# Offline self-test through the real binary
self-test: build
    ./target/debug/gosh-appimage-manager --self-test

# Desktop + AppStream validation
validate:
    desktop-file-validate data/com.goshapps.AppImageManager.desktop
    appstreamcli validate --pedantic --no-net data/com.goshapps.AppImageManager.metainfo.xml

# Regenerate packaging/cargo-sources.json with the official generator.
# Pass the flatpak-builder-tools checkout, e.g.:
#   just vendor /path/to/flatpak-builder-tools
vendor fbtools:
    python3 {{fbtools}}/cargo/flatpak-cargo-generator.py -o packaging/cargo-sources.json Cargo.lock

# Flatpak builds (single manifest covers BOTH arches; run each arch separately)
flatpak-x86_64:
    flatpak-builder --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=x86_64

flatpak-aarch64:
    flatpak-builder --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=aarch64
