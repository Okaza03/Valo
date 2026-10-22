# The Valo server API

This guide is for whoever runs the media server. It covers what Valo asks the server for, the JSON it expects back, and how to get every check on the app's server check to pass.

## Find what you're looking for

| I want to…                                                   | Read                                       |
| ------------------------------------------------------------ | ------------------------------------------ |
| Pick a way to serve my library, or run the bundled server    | [Setup](setup.md)                          |
| Understand a PASS/WARN/FAIL on the app's server check        | [The server check](server-check.md)        |
| Add sign-in (`/api/info`, `/api/login`, `/api/logout`, `/api/me`) | [Sign-in](auth.md)                    |
| Build the library JSON (movies, series, episodes, cast, …)   | [The library](library.md)                  |
| Sync resume points and watched marks between devices         | [Watch progress](progress.md)              |
| Serve video files and artwork                                | [Streams and images](streams.md)           |
| Manage users and rescan from the app's Admin page            | [Admin API](admin.md)                      |
| Fill the Admin dashboard: status, syncs, live log            | [Dashboard](admin-dashboard.md)            |
| Show watching analytics to admins                            | [Analytics](admin-analytics.md)            |
| Take requests through a Discord bot                          | [Requests and the Discord bot](admin-requests.md) |
| Invite or run the ready-made Valo Discord bot                | [The Valo Discord bot](discord-bot.md)     |
| Edit titles and fetch details from IMDb in the app           | [Library editing](admin-library.md)        |
| Group titles into collections (Batman, Harry Potter, …)      | [Collections](admin-collections.md)        |
| Add Stremio addons (catalogs, metadata, subtitles) for everyone | [Addons](admin-addons.md)               |
| Choose which rows everyone sees on the home page             | [Layout](admin-layout.md)                  |
| List and sign out a user's devices                           | [Sign-in](auth.md#get-apisessions-optional) |
| Copy a working server in Laravel, Express or Flask           | [Examples](examples.md)                    |
| Test the server by hand with curl                            | [Testing with curl](testing.md)            |
| Fix a specific check message                                 | [Troubleshooting](troubleshooting.md)      |

## All endpoints at a glance

| Endpoint                                | Needed                       | Docs                                  |
| --------------------------------------- | ---------------------------- | ------------------------------------- |
| `GET /api/info`                         | required, no sign-in         | [Sign-in](auth.md#get-apiinfo-required-no-sign-in) |
| `POST /api/login`                       | required when `auth` is on   | [Sign-in](auth.md#post-apilogin-required-when-auth-is-true) |
| `POST /api/logout`                      | optional                     | [Sign-in](auth.md#post-apilogout-optional) |
| `GET /api/me`                           | optional                     | [Sign-in](auth.md#get-apime-optional) |
| `GET /api/sessions`, `DELETE /api/sessions/<id>` | optional            | [Sign-in](auth.md#get-apisessions-optional) |
| `GET /api/library`                      | required                     | [The library](library.md)             |
| `GET /api/progress`, `POST /api/progress` | optional                   | [Watch progress](progress.md)         |
| Stream URLs (named by the library)      | required                     | [Streams and images](streams.md)      |
| `/api/admin/users`, `/api/admin/rescan` | optional                     | [Admin API](admin.md)                 |
| `/api/admin/status`, `/sync/…`, `/logs` | optional                     | [Dashboard](admin-dashboard.md)       |
| `/api/admin/analytics`                  | optional                     | [Analytics](admin-analytics.md)       |
| `/api/admin/requests`, `/api/requests`  | optional                     | [Requests](admin-requests.md)         |
| `/api/admin/movies/…`, `/api/admin/series/…` | optional                | [Library editing](admin-library.md)   |
| `/api/admin/collections`                | optional                     | [Collections](admin-collections.md)   |
| `GET /api/addons`, `/api/admin/addons`  | optional                     | [Addons](admin-addons.md)             |
| `GET /api/layout`, `/api/admin/layout`  | optional                     | [Layout](admin-layout.md)             |
| `/api/admin/subtitles/bulk`, `/api/admin/episodes/…` | optional        | [Library editing](admin-library.md)   |
| `GET /admin`                            | optional                     | [Admin API](admin.md#get-admin-optional) |

## Conventions

All API requests are sent with `Accept: application/json`. All paths are relative to the address the user types. If they type `192.168.1.20:8080/media`, the app requests `http://192.168.1.20:8080/media/api/info`. Only `http://` works for streaming (the bundled FFmpeg is built without TLS).

When sign-in is on, every request after `/api/login` carries `Authorization: Bearer <token>`, including streams and images on the same server.
