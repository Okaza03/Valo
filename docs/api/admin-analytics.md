# Analytics

[← Back to the Admin API](admin.md)

The Admin page's _Analytics_ section: plays per day, the most watched titles, and how much each user watched, over 7, 30 or 90 days or a year. It needs one endpoint. Without it, the section says _Not set up_ and links here.

## `GET /api/admin/analytics?days=30`

`days` is `7`, `30`, `90` or `365`. Admins only, under the [usual rules](admin.md#rules-for-every-request).

```json
{
  "plays": 128,
  "hours": 214.5,
  "active_users": 5,
  "days": [
    { "day": "2026-09-17", "plays": 3, "hours": 4.2 },
    { "day": "2026-09-18", "plays": 0, "hours": 0 }
  ],
  "top": [
    { "title": "The Office", "kind": "series", "plays": 41, "hours": 15.3, "users": 3 },
    { "title": "Inception", "kind": "movie", "plays": 4, "hours": 9.6, "users": 2 }
  ],
  "top_rated": [
    { "title": "Arcane", "kind": "series", "avg_rating": 9.1, "rating_count": 4 }
  ],
  "users": [
    { "name": "user", "plays": 60, "hours": 101.2, "movies": 12, "episodes": 48, "last_active": 1792103422 }
  ]
}
```

| Field          | Type   |                                                                                         |
| -------------- | ------ | --------------------------------------------------------------------------------------- |
| `plays`        | int    | Plays in the range                                                                      |
| `hours`        | number | Hours watched in the range                                                              |
| `active_users` | int    | Users who watched anything in the range                                                 |
| `days`         | array  | One entry per day of the range, **oldest first**, days without plays included: `day` (`YYYY-MM-DD`), `plays`, `hours` |
| `top`          | array  | Most played first, about 20 **of each kind**: `title`, `kind` (`"movie"`/`"series"`), `plays`, `hours`, optional `users` (how many different users played it). Episodes count toward their series. The app has Both / Movies / Series tabs, so send `kind` |
| `top_rated`    | array  | Optional, best first: `title`, `kind`, `avg_rating` (out of 10), `rating_count`, from users' own ratings. Shown as _Top rated_ |
| `users`        | array  | `name`, `plays`, `hours`, `last_active` (unix time), optional `movies` and `episodes` (how many of each they watched) |

Every field is optional; missing numbers show as 0 and missing lists as empty.

`hours` is up to you: time actually watched (from progress positions) or the full length of everything played. Use the same rule as your website so the numbers match.

## `GET /api/admin/analytics/users/<name>?days=30`: optional

Clicking a user under _By user_ opens their page. `<name>` is the username (URL-encoded), `days` as above; `404` for an unknown user. Without this endpoint the page says it isn't set up.

```json
{
  "name": "jo", "display_name": "Jo Ann", "admin": false,
  "plays": 60, "hours": 101.2, "movies": 12, "episodes": 48,
  "finished": 50, "in_progress": 10, "last_active": 1792103422,
  "days": [{ "day": "2026-09-17", "plays": 3, "hours": 4.2 }],
  "hours_of_day": [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 3, 5, 8, 9, 6, 2, 1],
  "weekdays": [5, 4, 6, 8, 12, 14, 11],
  "top": [{ "id": "s:Arcane", "title": "Arcane", "kind": "series", "plays": 9, "hours": 6.4 }],
  "genres": [{ "name": "Drama", "hours": 30.1 }],
  "recent": [
    { "type": "episode", "id": "e:…", "series_id": "s:Arcane", "title": "Arcane", "episode": "S01E02 · Some Mysteries Are Better Left Unsolved",
      "position": 1500, "duration": 2400, "completed": false, "updated": 1792103422 }
  ],
  "sessions": [{ "device": "Valo on jo-pc (Linux)", "ip": "192.168.1.20", "created": 1790000000, "last_used": 1792103422 }]
}
```

| Field          | Type   |                                                                     |
| -------------- | ------ | ------------------------------------------------------------------- |
| `plays`, `hours`, `days` | | As in the overview, for this user only                      |
| `movies`, `episodes` | int | Optional: how many of each they played                         |
| `finished`, `in_progress` | int | Optional: plays watched to the end (95 %) and not          |
| `hours_of_day` | array  | Optional: 24 counts, plays per hour of the day (server time), shown as _When they watch_ |
| `weekdays`     | array  | Optional: 7 counts, Monday first                                    |
| `top`          | array  | Most watched first, like the overview's; with `id` the title opens on click |
| `genres`       | array  | Optional: `name`, `hours`, most watched first                       |
| `recent`       | array  | Newest first: `type` (`movie`/`episode`), `id`, `series_id` for episodes, `title`, optional `episode` label, `position`, `duration` (seconds), `completed`, `updated` (unix time) |
| `sessions`     | array  | Optional: their signed-in devices: `device`, `ip`, `created`, `last_used` |

## Where the numbers come from

The simplest source is what you already store for [watch progress](progress.md): every saved entry is a play, its `updated` time says when, and its `position` says how much was watched. That's what the reference server does. It undercounts rewatches (a second viewing overwrites the first entry), which is fine for an overview.

For exact numbers, also write a row to a `plays` table each time `POST /api/progress` starts a new viewing (no entry yet, or the old one was finished or older than a few hours), and count those rows.

## Laravel example

```php
Route::get('/admin/analytics', function (Request $r) {
    $days = min(max((int) $r->query('days', 30), 1), 366);
    $since = now()->subDays($days - 1)->startOfDay();
    $rows = Progress::where('updated_at', '>=', $since)->with('user', 'item')->get();
    $hours = fn ($c) => round($c->sum(fn ($p) => min($p->position, $p->duration ?: $p->position)) / 3600, 1);
    return [
        'plays' => $rows->count(),
        'hours' => $hours($rows),
        'active_users' => $rows->pluck('user_id')->unique()->count(),
        'days' => collect(range(0, $days - 1))->map(function ($n) use ($since, $rows, $hours) {
            $day = $since->copy()->addDays($n)->toDateString();
            $on = $rows->filter(fn ($p) => $p->updated_at->toDateString() === $day);
            return ['day' => $day, 'plays' => $on->count(), 'hours' => $hours($on)];
        }),
        'top' => $rows->groupBy(fn ($p) => $p->item->seriesTitle() ?? $p->item->title)
            ->map(fn ($g, $t) => ['title' => $t, 'plays' => $g->count(), 'hours' => $hours($g)])
            ->sortByDesc('plays')->values()->take(20),
        'users' => $rows->groupBy('user.username')->map(fn ($g, $n) => [
            'name' => $n, 'plays' => $g->count(), 'hours' => $hours($g), 'last_active' => $g->max('updated_at')->timestamp,
        ])->values(),
    ];
})->middleware(['auth:sanctum', 'can:admin']);
```
