# The library

[← Back to the API guide](README.md)

## `GET /api/library`: required

Returns the whole catalog in the format below. When `auth` is `true`, answer `401` without a valid token. Any `401` while the app is running sends the user back to the sign-in screen.

## The library JSON

```json
{
  "movies": [ <movie>, … ],
  "series": [ <series>, … ]
}
```

Either list can be empty, missing or `null`. Any field can be left out or set to `null`. URLs can be absolute (`http://cdn/x.jpg`) or relative to the server (`/art/42/poster.jpg`).

### Movie

| Field          | Type                   | Needed       | Shown                                           | Example                  |
| -------------- | ---------------------- | ------------ | ----------------------------------------------- | ------------------------ |
| `title`        | string                 | **required** | everywhere                                      | `"Inception"`            |
| `stream`       | URL                    | **required** | play button ([how to serve it](streams.md))     | `"/api/stream/movie/42"` |
| `id`           | string or int          | recommended  | keeps resume points and _My list_ stable        | `"42"`                   |
| `year`         | int                    |              | tiles, details                                  | `2010`                   |
| `overview`     | string                 |              | details, banner                                 | `"A thief who…"`         |
| `poster`       | URL                    |              | tiles (2:3)                                     | `"/art/42/poster.jpg"`   |
| `backdrop`     | URL                    |              | banner, details (16:9)                          | `"/art/42/backdrop.jpg"` |
| `logo`         | URL                    |              | replaces the title text (transparent PNG)       | `"/art/42/logo.png"`     |
| `thumb`        | URL                    |              | wide cards like Continue watching (16:9, with the title on it); the backdrop stands in without it | `"/art/42/thumb.jpg"` |
| `genres`       | string[]               |              | genre rows and filter                           | `["Sci-Fi"]`             |
| `rating`       | string                 |              | age rating badge                                | `"PG-13"`                |
| `duration`     | number                 |              | runtime, in seconds                             | `8880`                   |
| `added`        | int                    |              | _Recently added_, Unix time                     | `1760000000`             |
| `released`     | string                 |              | details, `YYYY-MM-DD`                           | `"2010-07-16"`           |
| `director`     | string                 |              | details                                         | `"Christopher Nolan"`    |
| `cast`         | [Cast](#cast-member)[] |              | cast row                                        |                          |
| `score`        | number                 |              | out of 10                                       | `8.4`                    |
| `score_source` | string                 |              | next to the score                               | `"TMDB"`                 |
| `reviews`      | [Review](#review)[]    |              | critics' quotes                                 |                          |
| `budget`       | int                    |              | US dollars                                      | `160000000`              |
| `revenue`      | int                    |              | box office, US dollars                          | `836800000`              |
| `file`         | [File](#file-info)     |              | file details (read from the video when missing) |                          |
| `subtitles`    | [Subtitle](#subtitle)[] |             | the player's subtitle menu                      |                          |
| `imdb_id`      | string                 |              | requests: "already in the library"              | `"tt1375666"`            |
| `collection`   | string                 |              | A to Z: sorted under this name                  | `"Batman"`               |
| `collection_index` | int                |              | its place in the collection, `0` first          | `2`                      |

Titles with a `collection` sit together in A to Z under the collection's name, in `collection_index` order (oldest first when it's missing). So _Batman_ (1989), _Batman Returns_ and _The Batman_ end up side by side under B. `collection` may also be an object with a `name` (`{"name": "Batman"}`), as some databases store it. Admins can manage collections from the app; see [Collections](admin-collections.md).

### Series

Same as a movie, minus `stream`, `subtitles`, `director`, `budget`, `revenue` and `duration`, plus:

| Field      | Type                | Needed                              | Shown         | Example                         |
| ---------- | ------------------- | ----------------------------------- | ------------- | ------------------------------- |
| `seasons`  | [Season](#season)[] | **required** (at least one episode) | season picker |                                 |
| `creators` | string[]            |                                     | details       | `["Greg Daniels"]`              |
| `network`  | string              |                                     | details       | `"NBC"`                         |
| `status`   | string              |                                     | details       | `"Ended"`, `"Returning Series"` |
| `end_year` | int                 |                                     | `2005–2013`   | `2013`                          |

### Season

| Field      | Type                  |                                |
| ---------- | --------------------- | ------------------------------ |
| `number`   | int                   | `0` is shown as _Specials_, `999` as _Featurettes_ (episodes as "Featurette 2"; see [Featurettes](featurettes.md)) |
| `title`    | string                | Optional. Replaces "Season N". |
| `episodes` | [Episode](#episode)[] |                                |

### Episode

| Field      | Type               | Needed       |                             |
| ---------- | ------------------ | ------------ | --------------------------- |
| `stream`   | URL                | **required** |                             |
| `number`   | int                | recommended  | Episodes are sorted by this |
| `id`       | string or int      | recommended  | Keeps resume points stable  |
| `title`    | string             | recommended  |                             |
| `overview` | string             |              |                             |
| `still`    | URL                |              | Thumbnail, 16:9             |
| `duration` | number             |              | Seconds                     |
| `aired`    | string             |              | `YYYY-MM-DD`                |
| `score`    | number             |              | Out of 10                   |
| `file`     | [File](#file-info) |              |                             |
| `subtitles` | [Subtitle](#subtitle)[] |         | The player's subtitle menu  |

### Cast member

```json
{ "name": "Cillian Murphy", "character": "Jim", "photo": "/art/people/1.jpg" }
```

### Review

```json
{ "quote": "…", "source": "Reverse Shot", "score": "4.5 / 5" }
```

### Subtitle

```json
{
  "id": "9f6600f04462",
  "language": "en",
  "label": "English (SDH)",
  "format": "srt",
  "forced": false,
  "sdh": true,
  "url": "/api/subtitles/movie/461/9f6600f04462"
}
```

| Field      | Type   |                                                                                        |
| ---------- | ------ | -------------------------------------------------------------------------------------- |
| `url`      | URL    | **Required.** Serves the file. Gets the same `Authorization` header as streams.        |
| `language` | string | ISO 639-1 (`"en"`, `"nb"`), `"und"` when unknown. The app turns on the language picked last. |
| `label`    | string | Shown in the menu. Falls back to `language`.                                           |
| `format`   | string | `"srt"`, `"vtt"`, `"ass"` or `"ssa"`. Taken from the URL's extension when missing.     |
| `forced`   | bool   | Only the parts in another language. Not turned on by itself when a full track exists.  |
| `sdh`      | bool   | With sound descriptions. The app remembers whether this kind was picked.               |
| `id`       | string | Recommended; keep it stable across rescans.                                            |

Send the file as UTF-8 if you can (the desktop app also reads UTF-16 with a BOM, and falls back to Windows-1252). The [reference server](../../server/valo_server.py) picks subtitles up from a `Subs` folder in each movie's or series' folder.

### File info

```json
{
  "quality": "1080p",
  "container": "mp4",
  "video_codec": "h264",
  "audio_codec": "aac",
  "audio_channels": 6,
  "bitrate": 2600000,
  "size": 2090000000,
  "name": "28.Days.Later.2002.1080p.mp4",
  "path": "/movies/28 Days Later"
}
```

`bitrate` is in bits per second and `size` in bytes. When `file` is missing, the app reads what it can from the video stream itself.

## A complete example

```json
{
  "movies": [
    {
      "id": "42",
      "title": "Inception",
      "year": 2010,
      "overview": "…",
      "genres": ["Sci-Fi"],
      "rating": "PG-13",
      "duration": 8880,
      "added": 1760000000,
      "poster": "/art/42/poster.jpg",
      "backdrop": "/art/42/backdrop.jpg",
      "stream": "/api/stream/movie/42"
    }
  ],
  "series": [
    {
      "id": "7",
      "title": "The Office",
      "year": 2005,
      "overview": "…",
      "genres": ["Comedy"],
      "poster": "/art/7/poster.jpg",
      "backdrop": "/art/7/backdrop.jpg",
      "added": 1760000000,
      "seasons": [
        {
          "number": 1,
          "episodes": [
            {
              "id": "7-1-1",
              "number": 1,
              "title": "Pilot",
              "overview": "…",
              "duration": 1380,
              "still": "/art/7/1/1.jpg",
              "stream": "/api/stream/episode/123"
            }
          ]
        }
      ]
    }
  ]
}
```
