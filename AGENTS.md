I'm building Oceans Ink, a Flatpak GNOME gtk Instapaper GUI app in Rust.

I run Fedora Linux with GNOME 50. I have the Builder IDE. I am not an expert
at any of this stuff.

Instapaper API v2 (beware: announced 2026-09-29, don't assume v1 API):
- Announcement: https://blog.instapaper.com/2026/09/29/instapaper-api-v2/
- OpenAPI spec: https://www.instapaper.com/api/2/openapi.json
- Base URL: https://www.instapaper.com/api/2, OAuth 2 bearer token auth
- Lists: GET /bookmarks?section=home|liked|archive (limit/offset pagination,
  max 500)
- Archive/unarchive: POST /bookmarks/{id}/move with {"section": "archive"|"home"}
- Delete: DELETE /bookmarks/{id} (permanent, confirm first)
- Like/unlike: POST/DELETE /bookmarks/{id}/like

## Decisions so far

- App ID: net.hardscrabble.oceans-ink (display name "Oceans Ink")
- Stack: Rust, gtk4-rs (gtk4 0.10), libadwaita 0.8, meson, Flatpak
  (org.gnome.Platform//50 + rust-stable//25.08 extension)
- Access token: user pastes a personal access token; stored in the system
  keyring via the `libsecret` crate (blocking API, called off the main
  thread). Schema "net.hardscrabble.oceans-ink" with attribute
  app="oceans-ink". Note: libsecret 0.9 is built on glib/gio 0.22 while
  gtk4 0.10 uses 0.21, so secret.rs uses its own gio/glib 0.22 deps
- Flatpak cargo deps are vendored to gitignored build-aux/vendor; see
  README for the regenerate command
- OAuth flow for other users: punted for now

## Build

See README.md. Dev run: `OCEANS_INK_GRESOURCE=build/oceans-ink.gresource
./build/oceans-ink`

## jj workflow

- Run `jj new` and `jj desc` as you go, one change per logical unit of work,
  with accurate, succinct descriptions.
- Run `just check` (fmt-check + clippy + tests) before finishing each change
  and keep it green.
- Never commit credentials.

## Features

- prompt for access token and store it somewhere (done)
- Home lists unarchived articles
    - click to open the article in the default browser
    - right click to open a menu and archive, delete, or like
        - prompt for confirmation before deleting
    - j/k to move up and down thru the list
- Liked lists liked articles with same menu items
- Archive lists archived articles with same menu items
