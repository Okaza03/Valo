# My list and Continue watching

[← Back to the API guide](README.md)

With these endpoints, a user's _My list_ and the titles they took off _Continue watching_ follow them between the desktop app, the TV app and your website. They're optional. Without them, both stay on each device. Continue watching itself is built from [watch progress](progress.md), so it already matches everywhere once progress syncs. These endpoints add the "remove from row" part.

All four need a signed-in user, like [`/api/progress`](progress.md). Answer `401` when there's none. `401`, `404`, `405` or `501` on a `GET` turn this syncing off for the session.

## Titles

A title is `{"type": "movie" | "series", "id": …}`, with the `id` the [library](library.md) gives it. Whole-number ids are sent as numbers, others as strings. My list can also hold a title from an addon that isn't on the server: then it's `{"type": "movie", "addon_id": "tt1375666"}` with no `id`. Store whichever one you get, and send it back the same way.

## `GET /api/mylist`

Every change the user made to My list, newest first. Keep removals too (`"on": false`). Without them, a device that still has the title would add it back.

```json
[
  { "type": "movie", "id": 123, "on": true, "updated": 1792758941 },
  { "type": "series", "id": "7", "on": false, "updated": 1792700000 },
  { "type": "movie", "addon_id": "tt1375666", "on": true, "updated": 1792600000 }
]
```

| Field      | Type          |                                                             |
| ---------- | ------------- | ----------------------------------------------------------- |
| `type`     | string        | `"movie"` or `"series"`                                     |
| `id`       | string or int | The library's id                                            |
| `addon_id` | string        | Instead of `id`, for an addon's title                       |
| `on`       | bool          | `true`: on My list. `false`: taken off                      |
| `updated`  | int           | Unix time of the change, set by the server                  |

My list shows the titles with `"on": true`, newest first.

## `POST /api/mylist`

Adds a title to My list or takes it off. Store one entry per user and title, set `updated` to the current time, and answer with the saved entry in the `GET` shape. Answer `422` for an invalid body; the app drops that change instead of retrying it.

```json
{ "type": "movie", "id": 123, "on": true }
```

## `GET /api/hidden`

Titles the user took off Continue watching, newest first. `updated` is when. A title comes back on the row once it's watched again after that, so a later [progress](progress.md) entry for the movie (or any episode of the series) wins.

```json
[{ "type": "series", "id": 7, "updated": 1792758941 }]
```

## `POST /api/hidden`

Takes a title off Continue watching. Body: `{"type": "series", "id": 7}`. Set `updated` to the current time and answer with the saved entry. There's no un-hiding: watching the title brings it back. Answer `422` for anything but a movie or series from the library.

## How the apps sync

The apps work the same way as with progress. They ask for both lists on start, every minute and on _Refresh_, and take any entry newer than their own copy. They send each change right away, and changes made offline once the server answers again. A title that was on My list before syncing existed is sent once, the first time.
