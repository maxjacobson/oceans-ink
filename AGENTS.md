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
- Login flow: the token input screen is dev-mode only (MJ's personal
  access token). For other users the API v2 spec defines an OAuth 2
  authorization code flow: authorization URL
  https://www.instapaper.com/oauth2/authorize, token URL
  https://www.instapaper.com/oauth2/token, no scopes. Open questions to
  investigate: whether Instapaper supports PKCE (desktop apps cannot
  keep client secrets), what redirect URIs it allows (localhost loopback
  vs custom URL scheme), and whether there is a developer registration
  UI. Standard GNOME pattern when the time comes: register the app, open
  the authorize URL in the system browser, catch the redirect either via
  a temporary localhost listener or a custom scheme handler, exchange the
  code at the token URL, then store the access token in the keyring
  exactly as the dev token is stored today
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
- Edit without running rustfmt as you go (its reflowing breaks edit
  anchors); format once with `just fmt` right before committing, then run
  `just check` (fmt-check + clippy + tests) and keep it green.
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

## Features (current state)

- Token prompt stored in the keyring (done)
- Sidebar with Home / Liked / Archive, icons, and dim count badges
  (fetched with cheap limit=1 probes, refreshed after mutations)
- Lists show row thumbnails (cached to disk, see below) and red hearts on
  liked rows (hidden in the Liked list where it is implied)
- Click a row to drill into the in-app reader; context menu and keyboard
  shortcuts: j/k move (also j/k next/prev article while in the reader),
  Enter open, l like/unlike, y archive/unarchive,
  Backspace delete (confirm dialog), b open in browser,
  Ctrl+1/2/3 switch sections, Ctrl+? shortcuts, Ctrl+Q quit
- Toasts with Undo buttons for like/unlike/archive/move-to-home
- Scroll position is anchored to the topmost visible bookmark when
  drilling in, so returning (even after delete/archive) lands in place
- Deleting from the reader advances to the next bookmark in the list
  instead of popping back; pops only after deleting the last one
  (archive/unlike from the reader still pop back to the list)
- Thumbnail cache: src/thumbnail_cache.rs, files under the app cache dir
  named by URL SHA-256, atomically-written index.json with cached_at
  timestamps, pruned on startup past a 30-day cutoff (constant in the
  module; no pruning scheduler beyond that yet)
- Reader header buttons: heart, archive (undo-arrow icon in Archive),
  delete (trash), open-in-browser (custom globe); tooltips include the
  keyboard shortcuts
- Error page shows the article title plus the first 300 chars of an
  unexpected API response body (added to diagnose decode errors)

## Roadmap ideas

- Reading progress: API has progress (percentage + timestamp) on
  bookmarks and POST /bookmarks/{id} updateBookmark to set it
- Tags and folders: API has section=folder|tag with folder_id/tag params
  and /bookmarks/{id}/tags; once in-app, reconsider "Open in browser"
  pointing at instapaper.com/read/{id} (currently intentional so MJ can
  manage tags/folders on the website)
- Real login flow for other users: see the Login flow bullet under
  Decisions; replaces the dev-mode token prompt
- Request throttling/backoff: a burst of thumbnail + count requests once
  produced "error decoding response body" errors; possibly rate limiting.
  The decode-error diagnostics above will reveal the actual body if it
  recurs
- Pagination: list requests fetch limit=100 (BOOKMARKS_LIMIT in
  src/instapaper.rs) with no page-following yet
- Thumbnail cache eviction is time-based only; could also cap total size

## Dev workflow notes

- Dev loop: edit, cargo check, then `pkill -x oceans-ink; meson compile
  -C build; just run` (just run sets the session bus address and
  OCEANS_INK_GRESOURCE). cargo.sh copies the binary atomically, but a
  running instance still holds the old one; always relaunch after
  building and make sure only one window is open
- cargo test for unit tests (API client JSON parsing, thumbnail cache,
  scroll helpers)
- For visual debugging, MJ can drop a screenshot PNG into the project
  directory (gitignored) for the agent to view
- GTK gotchas learned: CSS cursor property did nothing (use
  widget.set_cursor_from_name); symbolic SVGs cannot rely on stroke or
  fill="none" because GTK's recoloring forces fill (draw outlines with
  even-odd compound paths); AdwNavigationSplitView's sidebar/content must
  be AdwNavigationPage (a bare widget renders blank)
