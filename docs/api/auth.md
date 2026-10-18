# Sign-in

[← Back to the API guide](README.md)

How the app finds the server, signs in and out, and learns who it's signed in as.

## `GET /api/info`: required, no sign-in

```json
{ "name": "Living-room server", "auth": true, "version": 1 }
```

| Field     | Type   |                                                                                                  |
| --------- | ------ | ------------------------------------------------------------------------------------------------ |
| `name`    | string | Shown in the title bar. Missing: the check warns and the host is shown.                          |
| `auth`    | bool   | `true` shows the sign-in screen. `false` (or missing): the check warns that the library is open. |
| `version` | int    | Send `1`.                                                                                        |

This must answer without a token, even when `auth` is `true`.

## `POST /api/login`: required when `auth` is `true`

Request body:

```json
{ "username": "user", "password": "hunter2", "device": "Valo on user-pc (Linux)" }
```

`device` names the app and machine, so the user can tell their devices apart in [`/api/sessions`](#get-apisessions-optional). Store it with the token if you list sessions; otherwise ignore it.

Success, `200`:

```json
{ "token": "3f1c…", "name": "user", "admin": false }
```

| Field   | Type   |                                                                                                                                                                        |
| ------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `token` | string | Required. Sent back as `Authorization: Bearer <token>` on every later request, including streams and images.                                                           |
| `name`  | string | Display name. Defaults to the username.                                                                                                                                |
| `admin` | bool   | `true` shows the app's Admin page (see [Admin API](admin.md)). Hiding it is only a convenience, so every admin endpoint must check permissions itself.                  |

Match the username **without regard to case**, so `user` signs in as `User`, and send the stored spelling back in `name`. The app keeps each user's local data (watch progress, My list) under the lowercased name, so both spellings are the same user either way. Then also refuse to create a second user whose name differs only in case.

A wrong username or password must get `401` (`400`, `403` and `422` work too). Don't answer `200` with an error message in it. The check sends an empty username and password and expects one of those codes, so a validation error for empty fields (Laravel's `422`) is fine.

## `POST /api/logout`: optional

Sent with the bearer token when the user signs out. Invalidate the token. The answer is ignored.

## `GET /api/me`: optional

Who the bearer token belongs to: `{"name": "user", "admin": true}`. A remembered session asks on every start, so admin rights given or taken away after signing in show up without signing in again. Without it, the app keeps what `/api/login` said.

Add `"must_change_password": true` when an admin asked the user for a new password ([`PATCH /api/admin/users/<name>`](admin.md#patch-apiadminusersname)). The app asks on every start until it's `false` again.

## `POST /api/password`: optional

The signed-in user changes their own password: `{"current_password": "…", "password": "…"}`. Answer `200` and clear `must_change_password`. Keep the token this request came with and end the user's other sessions. A wrong `current_password` gets `403`, an empty `password` `422` with `{"error": "…"}` (shown to the user). Without this endpoint, the app says the server can't change passwords from the app.

## `GET /api/sessions`: optional

Where the signed-in user is signed in. The profile page lists these under _Your devices_, and the user can sign any of them out. Without it, the page says the server doesn't list devices and links here.

```json
[
  { "id": "44def5fabd634374", "device": "Valo on user-pc (Linux)", "ip": "192.168.1.20", "created": 1792101466, "last_used": 1792103422, "current": true },
  { "id": "85f552dd6e5e91c6", "device": "Valo TV on SHIELD Android TV", "ip": "192.168.1.31", "created": 1791900000, "last_used": 1792000000, "current": false }
]
```

| Field       | Type        | Needed       |                                                                                       |
| ----------- | ----------- | ------------ | ------------------------------------------------------------------------------------- |
| `id`        | string      | **required** | Identifies the session in the URL below. **Never the token itself**: use a hash of it, or the token's database id |
| `device`    | string      |              | What the app sent as `device` when it signed in. Missing: "Unknown device"           |
| `ip`        | string      |              | Last address it was used from                                                         |
| `created`   | int         |              | Unix time it signed in                                                                |
| `last_used` | int         |              | Unix time it was last used                                                            |
| `current`   | bool        |              | `true` for the session this request came with. The app lists it first as _This device_ |

Only the caller's own sessions, `401` without a valid token.

### `DELETE /api/sessions/<id>`

Ends one of the caller's sessions (deletes that token). `200` on success, `404` when the id isn't one of theirs. When the user signs out _This device_, the app uses `/api/logout` instead.

Laravel with Sanctum:

```php
Route::middleware('auth:sanctum')->group(function () {
    Route::get('/sessions', fn (Request $r) => $r->user()->tokens->map(fn ($t) => [
        'id' => (string) $t->id, 'device' => $t->name, 'created' => $t->created_at?->timestamp,
        'last_used' => $t->last_used_at?->timestamp, 'current' => $t->id === $r->user()->currentAccessToken()->id,
    ]));
    Route::delete('/sessions/{id}', function (Request $r, string $id) {
        return $r->user()->tokens()->whereKey($id)->delete() ? [] : response()->json(['message' => 'Not found'], 404);
    });
});
// in /api/login: $user->createToken($request->input('device', 'Valo'))
```

## Tokens and 401s

When `auth` is `true`, every other endpoint (library, progress, streams, images, admin) must answer `401` without a valid token. Any `401` while the app is running sends the user back to the sign-in screen. Answer with JSON, not a redirect to a login page.
