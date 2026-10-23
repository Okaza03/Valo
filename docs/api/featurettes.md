# Featurettes

A series can keep its bonus videos (making-ofs, bloopers, interviews) in a `Featurettes` folder next to its seasons. The apps show it as one more season, **Featurettes**, after the real ones, and its videos as _Featurette 1_, _Featurette 2_, … wherever an episode code like _S01E02_ would go. They're left out of the season count ("2 seasons") and named in New arrivals ("Season 3 and Featurettes").

```
Series/
└── The Office/
    ├── Season 1/
    │   └── The.Office.S01E01.Pilot.mkv
    └── Featurettes/
        ├── Bloopers.mkv
        └── Making Of.mkv
```

The bundled server ([Setup](setup.md)) already does all of this. On your own server, the scan and _Sync series_ need the rules below. Without them, a sync treats the folder as a broken season: the files have no `SxxExx` in their names, so they're skipped, or the folder is renamed after its first file, and none of them gets artwork.

## In `/api/library`

Send the featurettes as a season numbered `999`, titled `Featurettes`:

```json
"seasons": [
  { "number": 1, "episodes": [ … ] },
  {
    "number": 999,
    "title": "Featurettes",
    "episodes": [
      { "id": "7-f-bloopers", "number": 1, "title": "Bloopers", "still": "/art/7/f/bloopers.jpg", "stream": "/api/stream/episode/130" },
      { "id": "7-f-making-of", "number": 2, "title": "Making Of", "still": "/art/7/f/making-of.jpg", "stream": "/api/stream/episode/131" }
    ]
  }
]
```

`999` is what the apps look for. The `title` is for older app versions, which show it in place of "Season 999".

## What the sync has to do

1. **Spot the folder first.** For each video under a series, check whether a folder between the series folder and the file is named `Featurettes` (any case) *before* you look for `SxxExx` in the file name. If it is, the file is a featurette, whatever it's called.
2. **Leave the files where they are.** Skip `Featurettes` in any step that renames or moves files into season folders, or names folders after episodes. The folder and its files keep the names the admin gave them.
3. **One season, number 999.** Put every featurette of the series into a season row numbered `999` with the title `Featurettes`. Create it the first time a featurette turns up, and delete it with the rest once its last file is gone, the same as any other empty season ([Removing titles](admin-dashboard.md#removing-titles)).
4. **Number them in name order.** Sort the series' featurettes by file name and number them `1`, `2`, `3`, …. Title each one after its file name, cleaned the way you clean show names: `Making.Of.1080p.mkv` becomes _Making Of_.
5. **Keep ids by path.** Match featurettes to their rows by file path, as you do for episodes, so the `id` (and the watch progress saved under it) stays the same when a new file sorts in front and the numbers shift. Only `number` changes.
6. **No online lookups.** TMDB, TVDB and OMDb have no season 999. Don't fetch season or episode details for it, and don't let a scheduled refresh overwrite a featurette's title with what such a lookup returns. Skip it in `POST /api/admin/series/<id>/seasons/<n>/refresh` too, or answer that there's nothing to fetch.
7. **Artwork from the video.** With no online still to download, take a frame from the video as its thumbnail (below), and save it wherever your other episodes' images go, through the same code. Name the file after the featurette's id or file name, never after an `SxxExx` code, which would make every featurette share `S999E01.jpg`.
8. **Report them like episodes.** New and removed featurettes go in the sync's `added` and `removed` lists, by file path.

A thumbnail with FFmpeg, 10% into the video:

```sh
ffmpeg -ss "$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$video" | awk '{print $1 * 0.1}')" \
  -i "$video" -frames:v 1 -vf scale=640:-2 -q:v 3 "$out.jpg"
```

## Laravel sketch

In the scanner, before the episode-code match:

```php
const FEATURETTES = 999;

foreach ($this->videos($seriesPath) as $file) {
    $relative = Str::after($file, $seriesPath . '/');
    $folders = array_map('strtolower', array_slice(explode('/', $relative), 0, -1));

    if (in_array('featurettes', $folders, true)) {
        $featurettes[] = $file;                    // never renamed, moved or looked up
        continue;
    }
    if (!preg_match('/s(\d{1,2})[ ._-]?e(\d{1,3})/i', basename($file), $m)) {
        $skipped[] = $file;
        continue;
    }
    $episodes[(int) $m[1]][(int) $m[2]] = $file;
}

if ($featurettes) {
    sort($featurettes, SORT_NATURAL | SORT_FLAG_CASE);
    $season = $series->seasons()->firstOrCreate(['number' => FEATURETTES], ['title' => 'Featurettes']);
    foreach (array_values($featurettes) as $i => $file) {
        $episode = $season->episodes()->updateOrCreate(
            ['path' => $file],                     // matched by path: ids never change
            ['number' => $i + 1, 'title' => Title::fromFile($file)],
        );
        if ($episode->wasRecentlyCreated) {
            $report->added[] = $file;
            MakeThumbnail::dispatch($episode->id); // a frame from the video, not a TMDB still
        }
    }
}
```

And in the jobs that fetch details:

```php
if ($season->number === FEATURETTES) {
    return;                                        // nothing online to fetch
}
```

## Check it

1. Add a `Featurettes` folder with two videos to a series and run _Sync series_. Both files are listed as new, and the folder and files keep their names.
2. Open the series in the app. The season picker has _Featurettes_ after the last season, each featurette has a thumbnail, and the header still counts only the real seasons.
3. Run _Sync series_ again. Nothing is new or removed, and no folder has been renamed.
4. Delete the folder and sync. The featurettes are removed and the _Featurettes_ season is gone.
