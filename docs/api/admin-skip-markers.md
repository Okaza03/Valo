# Skip markers

[← Back to the Admin API](admin.md)

While an episode plays, the player offers **Skip Recap** and **Skip Intro**. When the credits start, it offers **Next Episode**, which counts down 5 seconds. For that it needs to know where each episode's recap, intro and credits are.

- **From the file:** the app reads chapter names on its own: "Previously on", "Opening", "End Credits" and the like. Nothing on the server is needed for that.
- **From the server:** the server can find the times for files without named chapters, and send them with the library. The Admin page's _Skip markers_ section picks which series to look at.

## The library

Each episode can carry its times as `[start, end]` seconds. Leave out any that aren't known:

```json
{ "id": 812, "number": 3, "title": "…", "markers": { "recap": [0, 42.5], "intro": [42.5, 101], "credits": [1310, 1388] } }
```

The app uses these first. For any of the three the server leaves out, it falls back to the file's chapter names. Clear the library cache when times change.

## Finding the times

Two ways, best first:

1. **Chapter names**, read with ffprobe. They're exact, and the only reliable way to find a recap, since a recap's sound differs every episode.
2. **Matching sound between episodes.** Episodes of a season share their theme song and credits music. Take an audio fingerprint (Chromaprint: `ffmpeg … -f chromaprint -fp_format raw -`, or `fpcalc -raw`) of each episode's first ~10 minutes and last ~5. The longest stretch that matches a neighbouring episode's is the intro (15 s to 3 min) or the credits (at least 15 s). This is how Jellyfin's Intro Skipper works.

Detection can take a minute per season, so run it in the background, and only fill in times that are missing. Times an admin set by hand stay as they are.

## `GET /api/admin/skip-markers`

Every series with episodes, and how many of them have times:

```json
{
  "running": true,
  "series": [
    { "id": "7", "title": "Smallville", "year": 2001, "poster": "…", "episodes": 217,
      "intro": 210, "recap": 40, "credits": 214, "any": 215, "checked": 217,
      "job": { "state": "running" } }
  ]
}
```

- `any`: episodes with at least one time.
- `checked`: episodes detection has looked at, whether it found anything or not.
- `job`: the series' latest run, or null.
  - `state` is `queued`, `running`, `done` or `failed`.
  - `message` says what came of it ("215 of 217 episodes have times").
  - `at` is the Unix time it finished.

A series with `any` above 0 is listed under _Detected_ in the app; the rest under _Not detected_.

## `POST /api/admin/skip-markers/detect`

```json
{ "ids": [7, 12], "redo": false }
```

Queue these series and answer `202`. Series added while a run is going join it. `redo: true` finds the detected times again, but never the hand-set ones.

## `GET /api/admin/skip-markers/{id}`

The series' episodes, each with `recap`, `intro` and `credits` (`[start, end]` or null), plus:
- `source`: `chapters`, `audio`, `chapters,audio` or `manual`
- `checked`: Unix time

```json
{ "id": "7", "title": "Smallville", "job": null,
  "episodes": [{ "id": "812", "season": 1, "number": 3, "title": "Hothead",
                 "recap": null, "intro": [42.5, 101], "credits": [1310, 1388],
                 "source": "audio", "checked": 1794665041 }] }
```

## `PATCH /api/admin/skip-markers/episodes/{id}`

Set an episode's times by hand. Send only what changes; `null` clears one:

```json
{ "intro": [40, 98], "recap": null }
```

Answer with the episode's times as above. Hand-set times are marked `manual`, and detection leaves them alone. Answer `422` when one ends before it starts.

## `DELETE /api/admin/skip-markers/{id}`

Forget all of the series' times, the hand-set ones too.
