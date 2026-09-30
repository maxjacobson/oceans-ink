# Oceans Ink

An unofficial Instapaper client for GNOME, written in Rust.
App ID: `net.hardscrabble.oceans-ink`

> Vibecoded, just for fun. Not affiliated with, endorsed by, or connected to
> Instapaper in any way. Instapaper is a product of Filterpaper, LLC.

## Setup

Host build dependencies (Fedora package names):

```sh
sudo dnf install gtk4-devel libadwaita-devel libsecret-devel webkitgtk6.0-devel
```

Plus the usual tooling: a Rust toolchain (rustup), `meson`, `just`,
`git`, and `flatpak` with the flathub remote.

## Building and running

```sh
meson setup build
meson compile -C build
OCEANS_INK_GRESOURCE=build/oceans-ink.gresource ./build/oceans-ink
```

## Building as a Flatpak

```sh
cargo vendor build-aux/vendor > /dev/null
flatpak-builder --user --force-clean --install --state-dir=.flatpak-builder \
  flatpak-build build-aux/net.hardscrabble.oceans-ink.json
flatpak run net.hardscrabble.oceans-ink
```

Requires `org.gnome.Platform//50`, `org.gnome.Sdk//50`, and
`org.freedesktop.Sdk.Extension.rust-stable//25.08` (installed with
`flatpak install --user flathub <ref>`).

The Flatpak build compiles Rust offline from `build-aux/vendor`
(gitignored). Regenerate it with `cargo vendor build-aux/vendor > /dev/null`
whenever `Cargo.toml` or `Cargo.lock` changes.

## Layout

- `src/main.rs` — app entry, resource loading
- `src/window.rs` — main window (GTK composite template)
- `src/secret.rs` — access token storage via the system keyring
  (`libsecret` crate)
- `src/resources/` — GResource XML and UI templates
- `data/` — desktop file, appstream metainfo, icon
- `build-aux/` — Flatpak manifest (detected by Builder) and cargo.sh
