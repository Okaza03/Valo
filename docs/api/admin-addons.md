# Addons

[← Back to the Admin API](admin.md)

Admins can turn [Stremio](https://www.stremio.com/) addons on and off from a list, and every user gets what they offer:

| The addon offers | What users get                                                                                       | Examples                          |
| ---------------- | ---------------------------------------------------------------------------------------------------- | --------------------------------- |
| Catalogs         | Rows on the home page (_Popular · Movies_, _Netflix · TV shows_, …)                                   | Cinemeta, Streaming Catalogs, TMDB |
| Metadata         | Details a title on the server is missing (poster, backdrop, logo, overview, cast, genres, score, episode names and stills). Whatever the server has always wins | Cinemeta, The Movie Database Addon |
| Subtitles        | More subtitle tracks in the player, after the server's own (_English · OpenSubtitles v3_)              | OpenSubtitles v3                  |

**Streams are never used.** The videos always come from your server, so stream addons (torrents included) are refused, and an addon that offers streams next to other things is never asked for them.

Catalog titles that are on the server (same IMDb id) open and play as usual. The rest open a details page that says _Not on the server_, with a **Request** button. The request goes to [`POST /api/requests`](admin-requests.md) and shows up in the Admin page's _Requests_, marked _via App_. Without that endpoint, the page says the server doesn't take requests.

There are three parts:

1. **[The addon list](#the-addon-list)** is [`addons.json`](../../addons.json) in this repository, the same for every server. The Admin page shows it with a checkbox on each addon.
2. **`GET /api/addons`** lists the addons turned on for every signed-in user. The app talks to the addons itself.
3. **The admin endpoints** are what the checkboxes call. Without them, the _Addons_ section says _Not set up_ and links here.

## The addon list

Only the project's maintainer adds to the list. To get an addon on it, [open an addon request](https://github.com/Okaza03/Valo/issues/new?template=addon_request.yml); the _Request an addon_ button on the Admin page opens the same form.

To approve one, add it to `addons.json` on `main`. The app reads the file from GitHub, so it shows up on every server's Admin page without a release:

```json
{
  "name": "OpenSubtitles v3",
  "url": "https://opensubtitles-v3.strem.io/manifest.json",
  "description": "More subtitles in the player."
}
```

`name`, `url` and `description` are all needed, and `url` is the full `https://…/manifest.json`. Everyone can read the file, so a link can't hold an API key or other private settings. CI checks the file's format. It doesn't load the addon, so try the link before adding it: the checkbox shows the server's message if the addon is refused (see [below](#post-apiadminaddons)).

Taking an addon off the list doesn't turn it off anywhere. It stays on for servers using it, marked _Not on the list_, until an admin turns it off, and then it can't be turned back on. The same goes for addons added before the list existed.

## How the app uses addons

Titles are matched by IMDb id, so your movies and series need `imdb_id` in [the library](library.md) (on the Admin page: _Library_ › _Fetch from IMDb_). `<addon>` below is the manifest URL without `/manifest.json`.

| When                                         | Request to the addon                                       |
| -------------------------------------------- | ---------------------------------------------------------- |
| The library loads: each catalog              | `<addon>/catalog/movie/top.json`                           |
| A title on the server lacks a poster, backdrop or overview, or its page opens | `<addon>/meta/movie/tt1375666.json` |
| A title that isn't on the server opens       | `<addon>/meta/series/tt0903747.json`                       |
| A movie starts playing                       | `<addon>/subtitles/movie/tt1375666.json`                   |
| An episode starts playing                    | `<addon>/subtitles/series/tt0903747%3A1%3A2.json` (season 1, episode 2) |

The app only asks an addon for what its manifest says it offers, for those types and id prefixes. Metadata comes from the first addon in the list that has the title. Catalog rows show in the order the addons were added, up to 40 titles each. Catalogs that need a search or a filter aren't shown. The app never sends the user's Valo token to an addon.

If the user's last subtitle language (`en`, for example) isn't among the server's files but an addon has it, the player turns that track on once the addon answers.

## `GET /api/addons`

Any signed-in user. `200` with the list, oldest first; `[]` when there are none. A server without this endpoint (`404`) simply has no addons.

```json
[
  {
    "id": "ba3e09ce",
    "url": "https://v3-cinemeta.strem.io/manifest.json",
    "name": "Cinemeta",
    "description": "The official addon for movie and series catalogs",
    "version": "3.0.14",
    "types": ["movie", "series"],
    "resources": [
      { "name": "catalog", "types": ["movie", "series"], "id_prefixes": ["tt"] },
      { "name": "meta", "types": ["movie", "series"], "id_prefixes": ["tt"] }
    ],
    "catalogs": [
      { "type": "movie", "id": "top", "name": "Popular" },
      { "type": "series", "id": "top", "name": "Popular" }
    ],
    "added": 1792586293,
    "added_by": "boss"
  }
]
```

| Field         | Type     | Needed       |                                                                         |
| ------------- | -------- | ------------ | ----------------------------------------------------------------------- |
| `id`          | string   | **required** | Used in the delete URL. Base it on the `url` so it stays the same when the addon is turned off and on: [the layout](admin-layout.md) refers to rows by it |
| `url`         | string   | **required** | The addon's `manifest.json` URL. The app matches it against the list exactly, so keep it as it was sent |
| `name`        | string   | **required** | Shown in track names and on the Admin page                              |
| `description` | string   |              | Shown on the Admin page                                                 |
| `version`     | string   |              | Shown on the Admin page                                                 |
| `types`       | string[] |              | The manifest's `types`                                                  |
| `resources`   | object[] |              | What it offers, from the manifest: `catalog`, `meta` and/or `subtitles` (see below). Missing or empty: subtitles only |
| `catalogs`    | object[] |              | The rows: `type` (`movie` or `series`), `id` and `name` from the manifest's `catalogs` |
| `added`       | int      |              | Unix time                                                               |
| `added_by`    | string   |              | The admin who turned it on                                              |

Each resource has a `name`, the `types` it serves (empty: any) and the `id_prefixes` it knows (`null`: any). In the manifest a resource is either a string, which takes the manifest's own `types` and `idPrefixes`, or an object with its own.

## The admin endpoints

Admins only, under the [usual rules](admin.md#rules-for-every-request).

| Request                           | Body                | Answer                                          |
| --------------------------------- | ------------------- | ----------------------------------------------- |
| `POST /api/admin/addons`          | `{"url": "…"}`      | `201` with the new addon; `422` with a message when it's refused. Turns an addon on |
| `DELETE /api/admin/addons/<id>`   |                     | `200`; `404` for an unknown id. Turns an addon off |

### `POST /api/admin/addons`

The server fetches the manifest and checks it before adding the addon:

- `stremio://…` becomes `https://…`, and a URL without `/manifest.json` gets it added.
- The manifest needs an `id` and a `name`.
- **Refuse** with `422` when:
  - `behaviorHints.p2p` is `true`, for example `{"error": "torrent (P2P) addons aren't allowed"}`
  - `behaviorHints.configurationRequired` is `true`
  - `resources` has none of `catalog`, `meta` and `subtitles`. A stream-only addon such as Torrentio gets `{"error": "this is a stream addon; Valo plays from this server, so it doesn't take those"}`
  - the same manifest URL is already added
- Keep only the `catalog`, `meta` and `subtitles` resources, and only the `movie` and `series` catalogs that don't need a search or a filter (an `extra` with `"isRequired": true`, or `extraRequired`).

The app shows the `error` (or `message`) to the admin.
