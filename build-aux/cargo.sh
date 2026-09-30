#!/bin/sh
set -eu

src="$1"
build="$2"
out="$3"

export CARGO_TARGET_DIR="$build/target"

if [ -n "${FLATPAK_ID:-}" ]; then
  cargo build \
    --release \
    --manifest-path "$src/Cargo.toml" \
    --offline \
    --config 'source.crates-io.replace-with="vendored-sources"' \
    --config "source.vendored-sources.directory='$src/build-aux/vendor'"
else
  cargo build --release --manifest-path "$src/Cargo.toml"
fi

cp "$CARGO_TARGET_DIR/release/oceans-ink" "$out.tmp" && mv "$out.tmp" "$out"
