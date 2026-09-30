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
- In-app reader: clicking an article drills into a reader page (AdwNavigationView)
  with a WebKitGTK 6.0 WebView showing GET /bookmarks/{id}/parse body HTML, loaded
  against the article's original URL as base so relative images/links resolve.
  Links clicked inside the article open in the browser instead of navigating
  in-app. "Open in browser" in the context menu opens instapaper.com/read/{id}
  for now (useful for tags/folders until we implement them in-app)
- Beware: webkit6 crate 0.5 pairs with gtk4 0.10/glib 0.21; libsecret 0.9 pairs
  with glib 0.22, hence the two gio/glib versions in Cargo.toml

## Build

See README.md. Dev run: `OCEANS_INK_GRESOURCE=build/oceans-ink.gresource
./build/oceans-ink`

## jj workflow

- Run `jj new` and `jj desc` as you go, one change per logical unit of work,
  with accurate, succinct descriptions.
- Run `just check` (fmt-check + clippy + tests) before finishing each change
  and keep it green.
- Include a Crush attribution at the end of commit descriptions:
  "💘 Generated with Crush" plus "Assisted-by: Crush:glm-5.3-flash".
- `jj git push` only pushes bookmarks, so advance `main` to the newest
  described change before pushing. When the new work is the working-copy
  commit itself, that is `jj bookmark set main -r @` (using `@-` instead
  points at the previous commit and silently pushes nothing).
- Squashing into an already-pushed commit needs
  `jj squash --ignore-immutable` (pushed history is immutable by default).
  If a commit gets abandoned by mistake, `jj op log` shows the discarded
  commit id and `jj squash --from <id> --into <target> --ignore-immutable`
  brings the changes back.
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
