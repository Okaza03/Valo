# Dashboard

[← Back to the Admin API](admin.md)

The first section of the app's Admin page. Some of it works with no admin endpoints at all; the rest needs the endpoints below.

| Part              | Needs                                                                                     |
| ----------------- | ----------------------------------------------------------------------------------------- |
| Numbers (movies, series, episodes, size, users) | `library` in [`/api/admin/status`](#get-apiadminstatus). Without it, the app counts [`/api/admin/movies`](admin-library.md) (or `/api/library`) and [`/api/admin/users`](admin.md) |
| Library health    | `health` in `/api/admin/status`: your checks **replace** the app's, so it matches your website. Without them, the app checks for a missing poster, backdrop, overview, year and genres |
| API status        | [`GET /api/admin/status`](#get-apiadminstatus) for response time, version, uptime and services |
| Sync operations   | [`POST /api/admin/sync/<what>`](#post-apiadminsyncwhat), plus [`/api/admin/rescan`](admin.md#post-apiadminrescan) |
| Live log          | [`GET /api/admin/logs`](#get-apiadminlogs)                                                |

All of them follow the [rules for every admin request](admin.md#rules-for-every-request). `404`, `405` or `501` means "not set up", and the app links here.

## `GET /api/admin/status`

How the server and what it depends on are doing. The app asks every 30 seconds while the Dashboard is open and shows how long the answer took.

```json
{
  "version": "1.4.0",
  "uptime": 271800,
  "library": { "movies": 412, "series": 38, "episodes": 2210, "size": 5832110000000, "users": 6 },
  "services": [
    { "name": "Database", "ok": true, "detail": "PostgreSQL 16" },
    { "name": "IMDb lookups (OMDb)", "ok": true, "detail": "API key set" },
    { "name": "Discord bot", "ok": false, "detail": "last seen 2 hours ago" }
  ],
  "health": [
    { "label": "Files without a database entry", "count": 2, "items": ["/media/Movies/New (2024)/New.mkv"] },
    { "label": "Missing poster", "count": 1, "titles": [{ "type": "movie", "id": 12, "title": "Heat", "year": 1995 }] },
    { "label": "Database entries without a file", "count": 0 }
  ]
}
```

| Field      | Type     |                                                                                     |
| ---------- | -------- | ----------------------------------------------------------------------------------- |
| `version`  | string   | Your server's version                                                               |
| `uptime`   | int      | Seconds since it started                                                            |
| `library`  | object   | Your own numbers: `movies`, `series`, `episodes`, `size` (bytes), `users`. Each one is optional. **Any other number** in it becomes one more tile, labelled from its key: `"mp4": 271` shows as _MP4_, `"series_complete": 17` as _Series complete_ |
| `services` | array    | Anything it depends on: `name`, `ok` (green or red dot), optional `detail`         |
| `health`   | array    | Your library checks, in place of the app's: `label`, `count`, and what to list when the row is opened: `titles` (`{type, id, title, year}`, clickable: they open in the Library editor) and/or `items` (plain text, such as file paths). Send up to about 100 |

Every field is optional; `{}` is a valid answer. Good services to report: the database, the metadata source behind [Library editing](admin-library.md), the [Discord bot](admin-requests.md) (when it last called), free disk space, a transcoder.

## `POST /api/admin/sync/<what>`

The three sync buttons. `<what>` is one of:

| `<what>`  | Button       | What the server does                                                                                           |
| --------- | ------------ | -------------------------------------------------------------------------------------------------------------- |
| `folders` | Sync folders | Scans the media directories and compares them against the database. Lists new and removed files; **writes nothing** |
| `movies`  | Sync movies  | Adds new movie files to the database, removes movies whose files are gone, fetches details for the new ones    |
| `series`  | Sync series  | The same for series: new shows, seasons and episodes in, missing ones out, details for the new ones            |

Answer:

```json
{
  "added": ["/media/Movies/Dune Part Two (2024)/Dune.Part.Two.mkv"],
  "removed": ["/media/Movies/Old (1999)/Old.avi"],
  "updated": [],
  "applied": true,
  "running": false,
  "message": "1 new movie, 1 removed"
}
```

| Field     | Type     |                                                                                                  |
| --------- | -------- | ------------------------------------------------------------------------------------------------ |
| `added`   | string[] | New files (or titles). Shown in green                                                            |
| `removed` | string[] | Files that are gone. Shown in red                                                                |
| `updated` | string[] | Entries that changed                                                                             |
| `applied` | bool     | `false` for `folders` (a dry run); `true` when the database was changed. The app then reloads the library |
| `running` | bool     | `true` when the work carries on in the background. The app points the admin at the live log      |
| `message` | string   | One line shown above the lists                                                                   |

### Applying the folder sync

After a `folders` dry run that found changes, the app offers _Apply these changes_ (with a confirmation, since removed files lose their entries and watch progress). It sends `POST /api/admin/sync/folders?apply=1` and expects the same answer with `"applied": true`. Without `apply=1`, `folders` must never write anything.

The app waits up to 10 minutes for an answer. For anything slower, start the work, answer `{"running": true}` at once, and log progress to the [live log](#get-apiadminlogs).

`/api/admin/rescan` from the [Admin API](admin.md#post-apiadminrescan) shows up as a fourth button, _Rescan library_.

### Removing titles

An admin deletes a movie's or a series' folder, then runs _Sync movies_ or _Sync series_. Afterwards the title must be gone for good: not in the library, not in the database, and its folder must not come back. The apps then tell every user it's no longer available on their own; the server only has to stop listing it.

**1. Remove first, fetch details last.** In one sync, in this order:

1. Scan the folders. A title is _on disk_ only while its folder holds at least one video file. A folder left with just `poster.jpg`, `fanart.jpg` or `info.json` counts as gone.
2. Delete what isn't on disk anymore: a movie whose video file is gone; an episode whose file is gone; then a season with no episodes left and a series with no seasons left. Delete their rows (or soft-delete them so they're never listed again), with their artwork records, subtitles, collection entries and pending detail fetches.
3. Add what's new.
4. Fetch details and artwork **only for titles that are still in the database and on disk after step 2**. Never for something just removed, and never from a list of titles read before step 2.

**2. Never create a title's folder.** The folder belongs to the admin's files. When the server saves a poster, fanart, thumb or logo into it, it writes only if the folder already exists, and skips the image (with a log line) if it doesn't. No `mkdir -p`, no `Storage::put` into a missing folder: that's what brings a deleted series back as a folder of images. The same goes for writing `info.json`. Better still, keep downloaded artwork in the server's own storage (a cache folder or the database) rather than next to the videos; then a deleted folder can't be refilled at all.

**3. Don't let other jobs bring it back.** Queued jobs (detail fetches, artwork downloads, thumbnail generation) can run after the sync has removed the title. Each job loads the title again when it starts and stops if the title is gone or its folder is missing. A scheduled metadata refresh does the same.

**4. Stop listing it.** [`GET /api/library`](library.md) and `/api/admin/movies`/`series` mustn't return the title once it's removed. Don't cache the library for longer than a minute or so, and clear that cache at the end of every applied sync.

**5. Keep ids stable.** The apps tell titles apart by `id`. A sync must not delete and re-add titles that are still there under new ids, or every user sees them as removed and added again. Match existing rows by their path (or another stable key) and update them in place.

**6. Answer with what was removed.** List the removed files (or titles) in `removed`, with `"applied": true`, and log `sync series: 0 new, 1 removed`. The app reloads the library at once when it gets the answer.

**What users see.** The apps remember which titles each user has already seen, with their names. When a library loads without one of them, New arrivals shows it under _No longer on the server_ (or as _Removed from &lt;server&gt;_ if nothing new came with it), as soon as the user is back on Home, Movies or TV Shows. The apps reload the library after an admin sync, when someone comes back to the app after a minute or more, and, on desktop, every few minutes if the user turned that on in Settings. The server doesn't need anything else for this. Delete the removed title's watch progress along with it.

#### Laravel sketch

```php
public function syncSeries(bool $apply = true): SyncReport
{
    $onDisk = $this->scanner->series();            // [path => [episode paths]], only folders with video files
    $report = new SyncReport();

    DB::transaction(function () use ($onDisk, $report) {
        // 1. gone: episodes, then empty seasons, then empty series
        Episode::whereNotIn('path', Arr::flatten($onDisk))->each(function (Episode $e) use ($report) {
            $report->removed[] = $e->path;
            $e->delete();
        });
        Season::doesntHave('episodes')->delete();
        Series::doesntHave('seasons')->get()->each(function (Series $s) {
            $s->artwork()->delete();
            $s->collections()->detach();
            $s->delete();
        });
        // 2. new: match by path so existing ids never change
        foreach ($onDisk as $path => $episodes) {
            $series = Series::firstOrCreate(['path' => $path], ['title' => Title::fromFolder($path)]);
            // … add the missing seasons and episodes, $report->added[] = …
        }
    });

    // 3. details last, and only for what's still there
    Series::whereNull('details_fetched_at')->whereIn('path', array_keys($onDisk))
        ->each(fn (Series $s) => FetchDetails::dispatch($s->id));
    Cache::forget('library');
    return $report;
}

// in FetchDetails::handle(), and anywhere artwork is saved
$series = Series::find($this->id);
if (!$series || !is_dir($series->path)) {
    return;                                         // removed meanwhile: never recreate the folder
}
```

## `GET /api/admin/logs`

The live log. The app asks every 3 seconds while the Dashboard is open, with the last `id` it has:

```
GET /api/admin/logs?after=1041
```

```json
[
  { "id": 1042, "time": 1792103422, "level": "info", "message": "sync movies: 1 new, 0 removed" },
  { "id": 1043, "time": 1792103425, "level": "error", "message": "OMDb: request limit reached" }
]
```

| Field     | Type   |                                                                     |
| --------- | ------ | ------------------------------------------------------------------- |
| `id`      | int    | **required**: grows with every line                                 |
| `time`    | int    | Unix time                                                           |
| `level`   | string | `"debug"`, `"info"`, `"warning"` or `"error"`; colors the line      |
| `message` | string | One line of text                                                    |

Return only lines with an `id` greater than `after`, oldest first. With `after=0` (the first request), return the last 200 or so. Keep a ring buffer of the last thousand lines in memory; that's enough.

Don't log the log requests themselves, or the feed fills with its own polling. Never log passwords or tokens: admins see everything here.

## Laravel example

```php
Route::middleware(['auth:sanctum', 'can:admin'])->prefix('admin')->group(function () {
    Route::get('/status', fn () => [
        'version' => config('app.version'),
        'uptime' => now()->timestamp - (int) Cache::get('started_at', now()->timestamp),
        'services' => [
            ['name' => 'Database', 'ok' => rescue(fn () => DB::select('select 1') !== null, false)],
            ['name' => 'Discord bot', 'ok' => Cache::get('bot_seen', 0) > now()->subMinutes(10)->timestamp],
        ],
    ]);
    Route::post('/sync/{what}', function (string $what) {
        abort_unless(in_array($what, ['folders', 'movies', 'series']), 404);
        $r = app(LibrarySync::class)->run($what, dryRun: $what === 'folders');   // your scanner
        return ['added' => $r->added, 'removed' => $r->removed, 'applied' => $what !== 'folders'];
    });
    Route::get('/logs', fn (Request $r) => LogLine::where('id', '>', (int) $r->query('after', 0))
        ->orderBy('id')->limit(200)->get(['id', 'level', 'message', DB::raw('extract(epoch from created_at)::int as time')]));
});
```
