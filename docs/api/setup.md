# Setup: three ways to serve a library

[← Back to the API guide](README.md)

|                           | Setup                                         | Metadata                                          | Sign-in  |
| ------------------------- | --------------------------------------------- | ------------------------------------------------- | -------- |
| **Bundled Python server** | One command, no code                          | Everything, from `info.json` files                | Optional |
| **Your own API**          | Add four routes to an existing app            | Everything you send                               | Optional |
| **Plain folder listing**  | Turn on nginx/Apache/Caddy directory listings | Titles, years and artwork guessed from file names | None     |

## Option 1: the bundled Python server

`server/valo_server.py` implements the whole API with only the Python 3.9+ standard library.

```sh
python3 server/valo_server.py ~/Videos                     # port 8080, no sign-in
python3 server/valo_server.py ~/Videos --port 9000 --name "Living room"
python3 server/valo_server.py ~/Videos --add-user user     # asks for a password, turns sign-in on
python3 server/valo_server.py ~/Videos --add-user user --admin
python3 server/valo_server.py ~/Videos --omdb-key KEY --bot-token SECRET
```

`--add-user` creates the user and exits; `--admin` gives them the app's Admin page (users, dashboard, analytics, requests, library editing, collections). Start the server again afterwards. Users, tokens, each user's watch progress, requests and collections are stored in `~/Videos/.valo/`. Sign-in is on as soon as one user exists.

`--omdb-key` (or `OMDB_API_KEY`) turns on _Fetch from IMDb_ in the Admin page's Library section; get a free key at [omdbapi.com](https://www.omdbapi.com/apikey.aspx). `--bot-token` (or `VALO_BOT_TOKEN`) is the secret a [Discord request bot](admin-requests.md) uses to post requests. Edits made in the app are written to each title's `info.json`.

Put your files in `Movies/` and `Series/` (`Films/`, `TV/`, `TV Shows/` and `Shows/` also work):

```
Videos/
├── Movies/
│   └── Inception (2010)/
│       ├── Inception.2010.1080p.mkv
│       ├── poster.jpg          (or folder.jpg, cover.jpg, <video name>-poster.jpg)
│       ├── fanart.jpg          (or backdrop.jpg, background.jpg, <video name>-fanart.jpg)
│       ├── logo.png            (or clearlogo.png: transparent title artwork)
│       └── info.json
└── Series/
    └── The Office/
        ├── poster.jpg
        ├── fanart.jpg
        ├── info.json
        ├── Season 1/
        │   ├── The.Office.S01E01.Pilot.mkv
        │   └── The.Office.S01E01.Pilot.jpg   (episode thumbnail, or <name>-thumb.jpg)
        └── Featurettes/                      (shown as a "Featurettes" season, after the others)
            └── Bloopers.mkv                  (any file name; titled after it)
```

Everything else comes from `info.json`. Every key is optional:

```json
{
  "title": "Inception",
  "year": 2010,
  "overview": "A thief who steals corporate secrets through dream-sharing…",
  "genres": ["Sci-Fi", "Action"],
  "rating": "PG-13",
  "duration": 8880,
  "director": "Christopher Nolan",
  "cast": [
    {
      "name": "Leonardo DiCaprio",
      "character": "Cobb",
      "photo": "dicaprio.jpg"
    }
  ],
  "released": "2010-07-16",
  "budget": 160000000,
  "revenue": 836800000,
  "score": 8.4,
  "score_source": "TMDB",
  "reviews": [
    { "quote": "A wildly ambitious…", "source": "Empire", "score": "5 / 5" }
  ]
}
```

A series' `info.json` can also have `creators`, `network`, `status`, `end_year` and per-episode details:

```json
{
  "creators": ["Greg Daniels"],
  "network": "NBC",
  "status": "Ended",
  "end_year": 2013,
  "episodes": {
    "S01E01": {
      "title": "Pilot",
      "overview": "…",
      "aired": "2005-03-24",
      "score": 7.5
    }
  }
}
```

A cast `photo` can be a URL or an image file next to `info.json`. The server works out `added` (file date) and `file` (size, container, quality) on its own. Episode titles are taken from the file name when `info.json` doesn't give one.

## Option 2: your own API

Add the [endpoints](README.md#all-endpoints-at-a-glance) to an existing app. Only `/api/info`, `/api/library` and the stream URLs are required. Add `/api/login` if the library should be private, and `/api/progress` to sync watch progress between devices. See [Examples](examples.md) for Laravel, Express and Flask.

## Option 3: a plain folder listing

If `/api/info` is missing but the server lists its folders as HTML, Valo crawls the listing (up to 6 folders deep) and guesses movies and series from the names:

- `Show/Season 1/Show.S01E02.Title.mkv`, `Show.1x02.mkv`: episodes
- `Movies/Title (2010)/Title.mkv`, `Title.2010.1080p.mkv`: movies
- `poster.jpg`, `folder.jpg`, `fanart.jpg` next to the videos: artwork

The server must support HTTP range requests for seeking to work. nginx, Apache and Caddy do. `python -m http.server` doesn't.

```nginx
# nginx
location / {
    root /srv/videos;
    autoindex on;
}
```

```caddyfile
# Caddy
:8080 {
    root * /srv/videos
    file_server browse
}
```

```apache
# Apache
<Directory "/srv/videos">
    Options +Indexes
</Directory>
```

A folder listing can't give descriptions, genres, ratings, cast or a sign-in, so the server check shows these as warnings.
