# Recipes for Gosh AppImage Manager 3.0.0 (Rust core with a Flutter GUI).
# Run `just <recipe>`.

# Debug build of the Rust core and launcher (the GUI lives in flutter/)
build:
    cargo build

# Full test suite (fake seams + synthetic fixtures; no network, no home writes)
test:
    cargo test

# Flutter front end: format, analysis, and the Dart tests. The bridge tests drive
# the real core, so they run against a fresh scratch home, never a real one.
flutter-check:
    cd bridge && cargo build
    cd flutter && dart format --output=none --set-exit-if-changed lib test && flutter analyze && GOSHAIM_HOME="$(mktemp -d)" flutter test

lint:
    cargo clippy --all-targets -- -D warnings

fmt-check:
    cargo fmt --check

# Offline self-test through the real binary, with HOME and GOSHAIM_HOME in a scratch dir
self-test: build
    d="$(mktemp -d)"; HOME="$d" GOSHAIM_HOME="$d" ./target/debug/gosh-appimage-manager --self-test; rc=$?; rm -rf "$d"; exit $rc

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
