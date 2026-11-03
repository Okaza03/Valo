# Moving titles between disks

[← Back to the Admin API](admin.md)

While a movie's or a series' files are being moved from one disk to another, they're missing or half copied. The Admin page's _Moving_ section lets an admin mark a title as being moved:

- **Nobody can play it.** The apps grey out its Play button (_Unavailable — being moved_), and the server turns its streams down, so an app with an old library or a saved stream URL can't reach the half-copied files either.
- **Its artwork says so.** Every poster, thumbnail, backdrop and episode still of the title carries a red _MOVING_ band, in every app.
- **Unmarking it makes it playable again** once it plays from its new place.

Marking a series covers all of its episodes, Featurettes included.

## The library

Add `"unavailable": true` to every movie, series and episode being moved. The apps treat a series' flag as applying to all its episodes, so the episodes' own flag is optional. `"moving": true` is read the same way.

```json
{ "id": 7, "title": "Smallville", "unavailable": true, "seasons": [ … ] }
```

Clear the library cache when a flag changes, or the apps keep the old answer.

## Streams

While a title is marked, answer every stream request for it (its movie file or any of its episodes) with `503` before touching the file:

```json
{ "error": "Unavailable — this title is being moved. Try again later." }
```

Check before `X-Accel-Redirect`, `Range` handling or a transcoder, so nothing reaches the half-copied file. Block admins too, so they can check the lock works. Leave artwork and subtitles alone: the posters keep showing, with the band on them.

## `GET /api/admin/moving`

Every movie and series with its flag:

```json
{
  "movies": [{ "id": 12, "title": "Heat", "year": 1995, "moving": false }],
  "series": [{ "id": 7, "title": "Smallville", "year": 2001, "moving": true }]
}
```

Just the marked ids works too: `{"movies": [12, 45], "series": [7]}`. Ids are the ones the [library](library.md) uses. The app takes the titles themselves from the library, so it doesn't matter which shape you send.

## `PUT /api/admin/moving`

```json
{ "type": "series", "id": 7, "moving": true }
```

`type` is `"movie"` or `"series"`. `id` is a number when the library's id is one, else a string. Answer `200` (any body), or `404` when there's no such title. Log the change to the [live log](admin-dashboard.md#get-apiadminlogs), for example `Marked as moving (unavailable): Smallville` and `Available again: Smallville`.

Keep the marks somewhere that survives restarts and redeploys: a small JSON file (`{"movies": [12], "series": [7]}`) or a column in the database.

## Laravel sketch

```php
Route::get('/moving', fn () => [
    'movies' => Movie::get(['id', 'title', 'year'])->map(fn ($m) => $m->only('id', 'title', 'year') + ['moving' => MovingLock::has('movies', $m->id)]),
    'series' => Series::get(['id', 'title', 'year'])->map(fn ($s) => $s->only('id', 'title', 'year') + ['moving' => MovingLock::has('series', $s->id)]),
]);
Route::put('/moving', function (Request $r) {
    $kind = ['movie' => 'movies', 'series' => 'series'][$r->input('type')] ?? abort(422);
    MovingLock::set($kind, (int) $r->input('id'), (bool) $r->input('moving'));
    LibraryCache::clear();
    return [];
});

// in the stream controller, before anything else:
abort_if(MovingLock::has('series', $episode->series_id), 503, 'Unavailable — this title is being moved. Try again later.');
```

The [reference server](setup.md) keeps the marks in `.valo/moving.json`.
