# Upload

[← Back to the Admin API](admin.md)

The Admin page's _Upload_ section lets an admin put movies and episodes on the server from the desktop app. It shows the server's movie and series folders on every disk side by side, each with its free space. The admin picks one, goes into the folder they want (or makes one, e.g. `Heat.1995`, or `Dark` then `Season 01`), and chooses files to upload there.

Files go in chunks, one file at a time, on a thread of their own, so the admin can look at other pages meanwhile; the sidebar counts the uploads still going. Each file shows how far it got, its speed and the time left, and can be stopped. A dropped connection is retried. When a batch is done the app runs [`POST /api/admin/rescan`](admin.md#post-apiadminrescan), so the new titles show up in the library.

If Valo closes mid-upload, what arrived stays on the server. Picking the same file (same name and size) into the same folder again carries on from there. The section lists the server's unfinished uploads with a _Delete_ button for ones nobody will finish.

## The folders

Every movie and series folder the server is set up with is a _root_ (on the Valo Laravel server: `MOVIES_DIR` and `MOVIES_DIRS`, `SERIES_DIR` and `SERIES_DIRS`). The app only browses and uploads inside them, by a path relative to the root. The server must refuse any path that leaves its root: `..`, hidden names, and symlinks pointing elsewhere.

### `GET /api/admin/upload/folders[?root=<id>&path=<folder>]`

Without `root`, just the roots. With it, also what's in the folder `path` of that root (`""` or none: the root itself). Folders and files whose names start with `.` aren't listed.

```json
{
  "roots": [
    { "id": "movies-1", "kind": "movies", "disk": "Disk 1", "path": "/mnt/disk1/media/movies",
      "free_bytes": 812000000000, "total_bytes": 4000000000000, "writable": true },
    { "id": "series-1", "kind": "series", "disk": "Disk 1", "path": "/mnt/disk1/media/series", "…": "…" },
    { "id": "movies-0", "kind": "movies", "disk": "Disk 2", "path": "/media/movies", "…": "…" },
    { "id": "series-0", "kind": "series", "disk": "Disk 2", "path": "/media/series", "…": "…" }
  ],
  "chunk_size": 16777216,
  "root": "series-0",
  "path": "Dark",
  "folders": [{ "name": "Season 01" }, { "name": "Season 02" }],
  "files": [{ "name": "poster.jpg", "size": 482113 }]
}
```

| Field        |                                                                                         |
| ------------ | --------------------------------------------------------------------------------------- |
| `id`         | what the other calls name the root by                                                   |
| `kind`       | `movies` or `series`                                                                    |
| `disk`       | the disk it's on, for its label ("Disk 1 · Movies"); `null` when the server can't tell  |
| `free_bytes`, `total_bytes` | the disk's free and total space                                          |
| `writable`   | the server can write there; the app doesn't offer to upload when it can't               |
| `chunk_size` | the largest chunk the server takes in one request, in bytes                             |

`404` when the root or folder isn't there, `422` when the path isn't allowed.

### `POST /api/admin/upload/folders`

```json
{ "root": "series-1", "path": "", "name": "Severance" }
```

Makes the folder `name` in `path`. `201` with `{ "root": "series-1", "path": "Severance" }`; `409` when it's already there, `422` for a name with a slash or a leading dot.

## Uploading a file

1. `POST /api/admin/uploads` with where it goes, its name and size. The answer says how much already arrived (`offset`) and how big a chunk can be.
2. `PUT /api/admin/uploads/{id}?offset=N` with the next chunk as the body, until `offset` reaches `size`.
3. `POST /api/admin/uploads/{id}/finish` moves the whole file into place.

### `POST /api/admin/uploads`

```json
{ "root": "series-0", "path": "Dark/Season 01", "filename": "Dark.S01E01.1080p.mkv", "size": 2147483648 }
```

```json
{
  "id": "Xk2pQ9…", "root": "series-0", "path": "Dark/Season 01",
  "filename": "Dark.S01E01.1080p.mkv", "size": 2147483648,
  "offset": 0, "chunk_size": 16777216,
  "by": "boss", "started_at": "2026-11-21T10:00:00+00:00"
}
```

`201` for a new upload. When an unfinished upload is going to the same place with the same name and size, the server answers `200` with that one instead, so the app carries on from its `offset`.

| Status | When                                                               |
| ------ | ------------------------------------------------------------------ |
| `409`  | a file with that name is already in the folder                     |
| `422`  | not a video (mp4, mkv, avi, mov), subtitle (srt, vtt) or image (jpg, png), or no `size` |
| `507`  | the disk hasn't room for it                                        |

The Laravel server writes the chunks to a hidden `.<filename>.part` next to where the file goes, so finishing is a rename on the same disk, never a copy.

### `PUT /api/admin/uploads/{id}?offset=N`

The bytes from `N` on, as the body (`Content-Type: application/octet-stream`), at most `chunk_size` of them. `200` with `{ "offset": 33554432, "size": 2147483648 }`.

`409` with `{ "offset": … }` when `N` isn't where the upload is: a chunk sent twice after a dropped answer, or one lost. The app sends from that offset instead. `422` when the body goes past `size`.

### `GET /api/admin/uploads`, `GET /api/admin/uploads/{id}`

The unfinished uploads, oldest first, or one of them, shaped as above.

### `POST /api/admin/uploads/{id}/finish`

`200` with `{ "root": "series-0", "path": "Dark/Season 01/Dark.S01E01.1080p.mkv", "size": 2147483648 }`. `409` with `offset` when not all of it arrived, or when a file with that name appeared in the folder meanwhile.

### `DELETE /api/admin/uploads/{id}`

Stops it and deletes what arrived. `204`.

## Chunk size and proxies

Laravel refuses a request body bigger than PHP's `post_max_size`, PUT or not, and nginx one bigger than `client_max_body_size` (1 MB unless set). The Valo server's Docker images set `post_max_size` to 64 MB (`docker/php/uploads.ini`) and send chunks of `MEDIA_UPLOAD_CHUNK_MB` (16 by default), always below `post_max_size`. Any nginx or other proxy in front of it must allow a chunk too:

```nginx
client_max_body_size 64m;
```

When a proxy refuses a chunk (`413`), the app says so and names both settings.
