# Streams and images

[← Back to the API guide](README.md)

## Stream URLs: required

Each movie and episode names its own `stream` URL in the [library](library.md). The path is up to you, for example `/api/stream/movie/42`.

Stream URLs must serve the raw video file and support range requests:

- A request with `Range: bytes=0-1` gets `206 Partial Content` with a `Content-Range: bytes 0-1/<size>` header and those bytes.
- Send `Accept-Ranges: bytes` and a video `Content-Type` (`video/mp4`, `video/x-matroska`, or `application/octet-stream`).
- Never answer with an HTML page (a login page or a 404 page). The check fails on `text/html` and JSON.

Most frameworks do this for you: Laravel's `response()->file()`, Express's `res.sendFile()`, Flask's `send_file(..., conditional=True)`, nginx's `alias`/`root`.

A title an admin marked as being moved answers `503` instead; see [Moving](admin-moving.md).

The app transcodes nothing. It plays whatever FFmpeg can decode (H.264, HEVC, AV1, VP9, AAC, AC3, Opus, …) in any common container.

## Images

Images (posters, backdrops, logos, stills, cast photos) can be JPEG, PNG or WebP. When a sign-in is on, image and stream requests to the server carry the same `Authorization: Bearer` header. Images on another host (a CDN, TMDB) are fetched without it.

## Casting from the phone app

The server needs nothing extra for this. A Chromecast can't send the `Authorization` header, so the phone asks your server for the video and passes it on, and the TV only ever sees a random link to the phone. That has three consequences:

- The phone has to stay on and on the same Wi-Fi as the TV. A notification shows while it's casting, and swiping the app away ends it.
- Files are made playable on the way, like VLC does. MKV and other containers are rewrapped as MP4. If the TV refuses the sound (many Chromecast dongles only take stereo, so 5.1 AAC can play on a TV with Chromecast built in and be refused by a dongle), it becomes stereo. If the device can't decode the video (HEVC on an older Chromecast, or anything else that isn't H.264), the phone's hardware turns it into H.264, scaled down to 1080p if it's larger.
- This happens only while it plays: nothing is stored, and your files aren't touched. It needs a decoder on the phone for the file's formats, and converted video is less sharp than the original and takes more battery, so keep the phone plugged in for a long film. HDR10 is brought down to normal range on the way. The phone has to be able to decode the file itself: 4K 10-bit HEVC needs a hardware decoder.
- A converted stream can't jump, so a seek starts it again and takes a few seconds, most of it the Chromecast reloading.
- Subtitle files (SRT, VTT, ASS) are converted to WebVTT on the way, so the server's files and the addons' both show on the TV. Subtitles inside the video file don't, and their timing can't be moved there.
