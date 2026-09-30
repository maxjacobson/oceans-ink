default:
    just --list

build:
    meson compile -C build

run:
    export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}"
    OCEANS_INK_GRESOURCE=build/oceans-ink.gresource ./build/oceans-ink

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test

check: fmt-check lint test

install:
    flatpak-builder --user --force-clean --install --state-dir=.flatpak-builder \
        flatpak-build build-aux/net.hardscrabble.oceans-ink.json
