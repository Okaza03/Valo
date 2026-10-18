# Library editing

[← Back to the Admin API](admin.md)

The Admin page's _Library_ section lets an admin fix a title by hand (title, year, overview, genres, director or creators, age rating, score, IMDb id, poster, backdrop, thumbnail, logo) or fetch its details from IMDb and check them before saving. The title list and the "missing something" markers work without any of this. Editing needs the endpoints below; when the server answers `404`, `405` or `501`, the section says _Not set up_ and links here.

## URLs

`<kind>` is `movies` or `series`. `<id>` is the item's `id` from [`/api/library`](library.md), percent-encoded as one path segment (`m:Movies/Heat (1995)/Heat.mkv` becomes `m%3AMovies%2FHeat%20%281995%29%2FHeat.mkv`). Decode it after splitting the path on `/`.

| Request                                       | Body                          | Answer                                                      |
| --------------------------------------------- | ----------------------------- | ----------------------------------------------------------- |
| `GET /api/admin/<kind>`                       |                               | every title the admin manages, library-shaped               |
| `GET /api/admin/<kind>/<id>`                  |                               | one title with every field you keep                         |
| `PATCH /api/admin/<kind>/<id>`                | changed fields                | `200`; the app reloads the library                          |
| `POST /api/admin/<kind>/<id>/fetch`           | `{"title", "year", "imdb_id"}` | details found, library-shaped; **nothing is saved**        |
| `PUT /api/admin/<kind>/<id>/art/<which>`      | the image file                | `200`. `<which>` is `poster`, `backdrop`, `thumb` or `logo`          |
| `POST /api/admin/<kind>/<id>/art/refetch`     |                               | download only the missing images: `{"downloaded": 2, "kept": 2}` |
| `GET /api/admin/<kind>/<id>/tmdb-search?q=…`  |                               | `[{"tmdb_id", "title", "year", "poster", "overview"}]`     |
| `PUT /api/admin/<kind>/<id>/tmdb`             | `{"tmdb_id": 1197306, "refetch": true}` | `200`: match the title to that TMDB entry and refresh its details |
| `DELETE /api/admin/<kind>/<id>`               |                               | `200`: remove the title from the library                    |
| `GET /api/admin/<kind>/<id>/subtitles`        |                               | `[{"id", "language", "label", "source", "episode"}]`       |
| `PUT /api/admin/<kind>/<id>/subtitles?language=en[&episode=S01E02]` | the subtitle file | `200`                                     |
| `DELETE /api/admin/<kind>/<id>/subtitles/<sid>` |                             | `200`                                                       |
| `GET /api/admin/<kind>/<id>/subtitles/search?language=en[&episode=…]` |       | `[{"id", "name", "release", "language", "downloads", "episode", "moviehash_match"}]` ([picking one](#subtitle-search)) |
| `POST /api/admin/<kind>/<id>/subtitles/download[?episode=S01E02]` | `{"id": "<result id>", "episode": "S01E02"}` (`episode` for series: the one picked, else the result's) | `200`          |
| `POST /api/admin/subtitles/bulk`              | `{"type": "movies", "ids": […], "language": "en"}` | `202`; progress goes to the [live log](admin-dashboard.md#get-apiadminlogs) |
| `PATCH /api/admin/episodes/<id>`              | `{"title", "aired", "duration", "overview"}` | `200`. `aired` as `YYYY-MM-DD`, `duration` in seconds, `null` clears |
| `POST /api/admin/series/<id>/seasons/<n>/refresh` |                           | `200`: fetch the season's episode details again             |
| `POST /api/admin/episodes/<id>/fetch-image`   |                               | `200`: fetch the episode's still again                      |

Admins only, under the [usual rules](admin.md#rules-for-every-request). Errors with a short message (`{"error": "…"}` or `{"message": "…"}`) are shown to the admin.

## Subtitle search

A subtitle is timed to one release of a film: one made for a WEB or HDRip copy is often seconds off on a Blu-ray, and drifts when the frame rate differs (24 vs 23.976 fps). The most downloaded result is often not the right one. So:

- Send `release`, the video release the subtitle was made for (OpenSubtitles: `attributes.release`). The app compares it with the title's `file.name` and lists the ones that fit first, marked _Matches your file_.
- Send `moviehash_match: true` for results made for this exact file. Compute the [OpenSubtitles movie hash](https://trac.opensubtitles.org/projects/opensubtitles/wiki/HashSourceCodes) of the video and pass it as `moviehash` to their search; they flag the hits with `attributes.moviehash_match`. These go to the top.
- `POST /api/admin/subtitles/bulk` should pick the same way: a `moviehash_match` result first, then one whose release has the same group and source (BluRay, WEB, …) as the file, and only then the most downloaded.

## `GET /api/admin/movies`, `GET /api/admin/series`

Every title, as your own admin website lists them: the same shape as [`/api/library`](library.md), but **including titles users don't see**, such as ones without a file yet or hidden ones. The Library section's list, the Collections section's "Add titles" list and the Dashboard's numbers and health checks all use it, so the app shows the same titles as your website. Without it, they fall back to `/api/library` and the list says so.

Extra fields the list may carry:

| Field               | Type     |                                                                         |
| ------------------- | -------- | ----------------------------------------------------------------------- |
| `has_subtitles`     | bool     | Movies: at least one subtitle. With it (or the next one), the list gets an _Only titles missing subtitles_ filter and a bulk download |
| `missing_subtitles` | int      | Series: episodes without a subtitle (`missing_subtitle_count` works too) |
| `locked`            | string[] | Fields an admin edited, which syncs leave alone; shown under the fields |
| `tmdb_id`           | int      | Shown in the TMDB match panel                                           |

The `id`s must be the same as in `/api/library`. Leave out the heavy parts the admin pages don't need (`seasons`, `cast`, `reviews`) if the list gets big.

## `GET /api/admin/<kind>/<id>`

One title, with every field your database keeps for it, in the library's names. Put the video file's details in `file` (`name`, `path`, `quality`, `container`, `video_codec`, `audio_codec`, `audio_channels`, `bitrate`, `size`, the [File info](library.md#file-info) shape) for the editor's _Files_ tab; a series gets `seasons` with `episodes` (each with its own `file`) for the _Files_ and _Episodes_ tabs. The editor opens with these values, so fields that `/api/library` leaves out (`imdb_id`, an unpublished overview, …) show up and aren't lost on save. Without it, the editor uses the list's copy.

## `PATCH /api/admin/<kind>/<id>`

Only the fields the admin changed, with the [library's names](library.md#movie):

```json
{
  "title": "Batman",
  "year": 1989,
  "overview": "The Dark Knight of Gotham City begins his war on crime…",
  "genres": ["Action", "Crime"],
  "director": "Tim Burton",
  "score": 7.5,
  "imdb_id": "tt0096895",
  "poster": "https://m.media-amazon.com/images/M/….jpg",
  "rating": null
}
```

- `null` clears a field.
- Movies may send `title`, `year`, `overview`, `genres`, `rating`, `director`, `released`, `score`, `imdb_id`, `poster`, `backdrop`, `thumb`, `logo`.
- Series may send the same, with `creators` (a list) instead of `director`, plus `network`, `status`, `end_year`.
- `poster`, `backdrop`, `thumb` and `logo` are image addresses the admin pasted (or that a fetch found). **Download them** and serve them like the rest of your artwork; don't hand the remote URL to every client. They're only sent when the admin changed them, so the current artwork's URL never comes back. `null` means the admin removed it: delete the image.

Store the edits so a rescan or a sync doesn't overwrite them (for example, mark edited fields as locked, or keep edits in their own table that wins over scanned data).

## `POST /api/admin/<kind>/<id>/fetch`

The _Fetch from IMDb_ button.

```json
{ "title": "Batman", "year": 1989, "imdb_id": "tt0096895" }
```

`imdb_id` is sent when the admin filled it in; look the title up by it then, else by `title` and `year`. Answer with what you found, in the library's shape:

```json
{
  "title": "Batman",
  "year": 1989,
  "overview": "The Dark Knight of Gotham City begins his war on crime…",
  "genres": ["Action", "Adventure"],
  "rating": "PG-13",
  "director": "Tim Burton",
  "released": "1989-06-23",
  "score": 7.5,
  "score_source": "IMDb",
  "imdb_id": "tt0096895",
  "poster": "https://m.media-amazon.com/images/M/….jpg"
}
```

The app fills the editor with every field that isn't empty and the admin saves with `PATCH`, so this endpoint must not change anything. `422` with a message when nothing was found (`{"error": "Movie not found!"}`); `501` when lookups aren't configured (the app then hides the button behind a link here).

IMDb itself has no public API. Ways to get its data:

- [OMDb](https://www.omdbapi.com/apikey.aspx): IMDb's data, free key (1,000 requests a day). The reference server uses it: `--omdb-key` or `OMDB_API_KEY`.
- [TMDB](https://developer.themoviedb.org/docs): free key, better posters and backdrops, has the IMDb id (`/find/{imdb_id}?external_source=imdb_id`).
- [IMDb's datasets](https://developer.imdb.com/non-commercial-datasets/): daily TSV files, no images.

## The editor's tabs

| Tab       | Needs                                                                                      |
| --------- | ------------------------------------------------------------------------------------------ |
| Details   | `PATCH`, `fetch`; under the fields, the TMDB match (`tmdb-search`, `PUT …/tmdb`) and _Delete title_ (`DELETE`) |
| Images    | `PATCH` (addresses, `null` removes), `PUT …/art/<which>`, `POST …/art/refetch`             |
| Files     | `file` (and `seasons`) in `GET /api/admin/<kind>/<id>`                                     |
| Subtitles | the `…/subtitles` endpoints                                                                |
| Episodes  | series only: `seasons` in the record, `PATCH /api/admin/episodes/<id>`, the season refresh and still fetch |

Episode `<id>`s are the ids `/api/library` gives episodes. Any of these the server answers with `404`, `405` or `501` shows "This server doesn't support that yet" and a link here; the rest of the editor keeps working.

## `PUT /api/admin/<kind>/<id>/art/<which>`

The _Upload…_ buttons. The body is the image file, with `Content-Type: image/jpeg`, `image/png` or `image/webp`, up to about 25 MB. Replace the old image (and any other format of it, so a stale `poster.jpg` doesn't win over the new `poster.png`). A logo should be a PNG with a transparent background.

Afterwards the app reloads the library and downloads the image again, even if its URL didn't change.

## Reference server

Edits go into the title's `info.json` (the movie's own folder, or `<file>.json` for a loose file; the show's folder for a series). Artwork is saved as `poster.*`, `fanart.*`, `thumb.*` and `logo.png` in the folder (`<file>-poster.*`, `<file>-fanart.*` and `<file>-thumb.*` for a loose movie file, which can't have a logo).

## Laravel example

```php
Route::middleware(['auth:sanctum', 'can:admin'])->prefix('admin')->group(function () {
    Route::patch('/movies/{movie}', function (Request $r, Movie $movie) {
        $data = $r->only(['title', 'year', 'overview', 'genres', 'rating', 'director', 'released', 'score', 'imdb_id']);
        foreach (['poster', 'backdrop', 'logo'] as $art) {
            if ($url = $r->input($art)) {
                $movie->{$art.'_path'} = Storage::disk('public')->putFile('art', Http::get($url)->toPsrResponse()->getBody());
            }
        }
        $movie->fill($data)->forceFill(['locked' => array_keys($data)])->save();
        return [];
    });
    Route::post('/movies/{movie}/fetch', function (Request $r, Movie $movie) {
        $q = $r->filled('imdb_id') ? ['i' => $r->imdb_id] : ['t' => $movie->title, 'y' => $movie->year, 'type' => 'movie'];
        $d = Http::get('https://www.omdbapi.com/', $q + ['apikey' => config('services.omdb.key'), 'plot' => 'full'])->json();
        abort_if(($d['Response'] ?? '') !== 'True', 422, $d['Error'] ?? 'Not found');
        return ['title' => $d['Title'], 'year' => (int) $d['Year'], 'overview' => $d['Plot'], 'rating' => $d['Rated'],
                'genres' => array_map('trim', explode(',', $d['Genre'])), 'director' => $d['Director'],
                'imdb_id' => $d['imdbID'], 'score' => (float) $d['imdbRating'], 'poster' => $d['Poster']];
    });
    Route::put('/movies/{movie}/art/{which}', function (Request $r, Movie $movie, string $which) {
        abort_unless(in_array($which, ['poster', 'backdrop', 'logo']), 404);
        $path = 'art/'.Str::uuid().'.'.(['image/png' => 'png', 'image/webp' => 'webp'][$r->header('Content-Type')] ?? 'jpg');
        Storage::disk('public')->put($path, $r->getContent());
        $movie->update([$which.'_path' => $path]);
        return [];
    });
    // the same three for /series/{series}
});
```
