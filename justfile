default:
    just --list

build:
    meson compile -C build

run:
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
