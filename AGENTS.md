I'm interested in building a Flatpak GNOME gtk Instapaper GUI app.

I am running on Fedora Linux with GNOME 50.

I have the Builder IDE I can use.

I am not an expert at any of this stuff.

FYI they just announced v2 so make sure to avoid assumptions about the API.

- Announcement: https://blog.instapaper.com/2026/09/29/instapaper-api-v2/
- OpenAPI spec: https://www.instapaper.com/api/2/openapi.json

Let's plan to write Rust lang.

## jj workflow

- Run `jj new` and `jj desc` as you go, one change per logical unit of work,
  with accurate, succinct descriptions.
- Never commit credentials.

Feel free to rewrite this file as you build this out and make decisions.

## Credentials

I've registered an app, and I have a personal access token we can use during development.

I guess for other users we'll need to do an oauth flow or something? We can punt on that for now.

## Features

- prompt for access token and store it somewhere
- Home lists unarchived articles
    - click to open the article in the default browser
    - right click to open a menu and archive, delete, or like
        - prompt for confirmation before deleting
    - j/k to move up and down thru the list
- Liked lists liked articles with same menu items
- Archive lists archived articles with same menu items
