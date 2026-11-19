# Images

[← Back to the Admin API](admin.md)

The Admin page's _Images_ section follows **Upgrade images**: the job that checks every movie's and series' poster, backdrop, thumbnail and logo against TMDB and downloads the largest version where it's bigger than the image there, or the image is missing. Images an admin set by hand are kept. The movie and series syncs only fetch images that are missing.

The section shows:

- the run going on now, with a progress bar that updates every few seconds;
- the last run's totals (upgraded, added, already the largest, failed), every image it changed (old size → new size, clickable to open the title in the Library editor) and every failure with its reason;
- earlier runs' totals;
- the library's images by resolution, per slot: how many are 4K, 1440p, 1080p, 720p, SD or missing, and how many were set by hand.

The run itself is started with [`POST /api/admin/sync/images`](admin-dashboard.md#post-apiadminsyncwhat), the same as the Dashboard's _Upgrade images_ button. The Dashboard also shows the last run's upgraded count from [`image_upgrades`](admin-dashboard.md#get-apiadminstatus).

## `GET /api/admin/images`

```json
{
  "running": false,
  "current": null,
  "last": {
    "started_at": "2026-11-19T19:00:00+00:00",
    "finished_at": "2026-11-19T19:42:10+00:00",
    "total": 450, "checked": 450,
    "upgraded": 812, "added": 37, "kept": 950, "failed": 3,
    "changes": [
      { "title": "Heat", "kind": "movie", "id": 12, "slot": "poster", "from": [500, 750], "to": [2000, 3000], "tier": "1440p" },
      { "title": "Arcane", "kind": "series", "id": 4, "slot": "backdrop", "from": null, "to": [3840, 2160], "tier": "4K" }
    ],
    "failures": [
      { "title": "Dark", "kind": "series", "id": 9, "slot": "logo", "reason": "couldn't download it" }
    ]
  },
  "runs": [
    { "started_at": "…", "finished_at": "…", "total": 450, "checked": 450, "upgraded": 812, "added": 37, "kept": 950, "failed": 3 }
  ],
  "library": {
    "poster":   { "4K": 0,   "1440p": 410, "1080p": 30, "720p": 4, "SD": 2, "missing": 4, "locked": 6 },
    "backdrop": { "4K": 380, "1440p": 50,  "1080p": 15, "720p": 0, "SD": 0, "missing": 5, "locked": 1 },
    "thumb":    { "4K": 300, "1440p": 90,  "1080p": 50, "720p": 3, "SD": 0, "missing": 7, "locked": 0 },
    "logo":     { "4K": 2,   "1440p": 40,  "1080p": 160, "720p": 180, "SD": 50, "missing": 18, "locked": 2 }
  },
  "tmdb": true
}
```

| Field     |                                                                                                   |
| --------- | ------------------------------------------------------------------------------------------------- |
| `running` | a run is going on now                                                                            |
| `current` | that run: `started_at`, `total` titles, `checked` so far, and the counts so far. `null` when none is running; a run whose progress stopped (killed, crashed) shouldn't count as running |
| `last`    | the last finished run: its counts, `changes` and `failures`. `null` before the first one          |
| `runs`    | earlier runs' totals, newest first (no lists)                                                     |
| `library` | per slot (`poster`, `backdrop`, `thumb`, `logo`): images by resolution, judged by the long edge (4K at 3840 px or more, 1440p at 2560, 1080p at 1920, 720p at 1280, else SD), `missing`, and `locked` (set by hand) |
| `tmdb`    | the server has a TMDB key; without one, the app doesn't offer to run it                           |

In `changes`, `from` is the replaced image's size (`null`: there was none), `to` the new file's size, `tier` its resolution. `kind` and `id` name the title as in [`/api/admin/movies`](admin-library.md) and `/api/admin/series`; the app opens it in the Library editor.

`?fresh=1` asks for the library counts again instead of from a cache: counting reads every image file's header, so cache it for a few minutes and drop the cache when a run finishes.
