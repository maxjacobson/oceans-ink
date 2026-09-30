# Oceans Ink

An unofficial Instapaper client for GNOME, written in Rust.
App ID: `net.hardscrabble.oceans-ink`

> [!NOTE]
> This is a vibecoded, just for fun little project. I am sure it is very
> imperfect in many ways, but it's kind of fun to be able to conjure things
> like this into existence.

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
just install
flatpak run net.hardscrabble.oceans-ink
```

Which runs:
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
