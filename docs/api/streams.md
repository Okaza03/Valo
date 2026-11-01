# Streams and images

[← Back to the API guide](README.md)

## Stream URLs: required

Each movie and episode names its own `stream` URL in the [library](library.md). The path is up to you, for example `/api/stream/movie/42`.

Stream URLs must serve the raw video file and support range requests:

- A request with `Range: bytes=0-1` gets `206 Partial Content` with a `Content-Range: bytes 0-1/<size>` header and those bytes.
- Send `Accept-Ranges: bytes` and a video `Content-Type` (`video/mp4`, `video/x-matroska`, or `application/octet-stream`).
- Never answer with an HTML page (a login page or a 404 page). The check fails on `text/html` and JSON.

Most frameworks do this for you: Laravel's `response()->file()`, Express's `res.sendFile()`, Flask's `send_file(..., conditional=True)`, nginx's `alias`/`root`.

The app transcodes nothing. It plays whatever FFmpeg can decode (H.264, HEVC, AV1, VP9, AAC, AC3, Opus, …) in any common container.

## Images

Images (posters, backdrops, logos, stills, cast photos) can be JPEG, PNG or WebP. When a sign-in is on, image and stream requests to the server carry the same `Authorization: Bearer` header. Images on another host (a CDN, TMDB) are fetched without it.

## Casting from the phone app

The server needs nothing extra for this. A Chromecast can't send the `Authorization` header, so the phone asks your server for the video and passes it on, and the TV only ever sees a random link to the phone. That has three consequences:

- The phone has to stay on and on the same Wi-Fi as the TV. A notification shows while it's casting, and swiping the app away ends it.
- The file has to be something the Chromecast can play, since nothing is transcoded: H.264 video (4K models HEVC too) with AAC or MP3 sound. The app says so when the TV refuses a file.
- Subtitle files (SRT, VTT, ASS) are converted to WebVTT on the way, so the server's files and the addons' both show on the TV. Subtitles inside the video file don't, and their timing can't be moved there.
