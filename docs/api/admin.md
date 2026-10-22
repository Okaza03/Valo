# Admin API

[← Back to the API guide](README.md)

The app's Admin page has a sidebar of sections. Each one needs its own endpoints, and all of them are optional: a section the server doesn't support says _Not set up_ and links to its guide.

| Section     | What it does                                                              | Guide                                       |
| ----------- | ------------------------------------------------------------------------- | ------------------------------------------- |
| Dashboard   | Library numbers, API status, library health, syncs, live log              | [Dashboard](admin-dashboard.md)             |
| Analytics   | Plays per day, most watched titles, each user's watching                  | [Analytics](admin-analytics.md)             |
| Requests    | Titles people asked for through a Discord bot                             | [Requests and the Discord bot](admin-requests.md) |
| Library     | Edit a title's details and artwork, fetch them from IMDb                  | [Library editing](admin-library.md)         |
| Collections | Group titles so A to Z keeps them together, in an order the admin picks   | [Collections](admin-collections.md)         |
| Addons      | Stremio addons for everyone: home-page catalogs, missing details, subtitles | [Addons](admin-addons.md)                 |
| Layout      | Which rows everyone sees on the home page, and their order                  | [Layout](admin-layout.md)                 |
| Users       | List, add and remove users, admin rights, passwords, sign out everywhere  | this page                                   |

This page covers the Users section and the rules every admin endpoint follows. The library numbers and health checks on the Dashboard work with no admin endpoints at all; they come from the library.

The bundled [reference server](setup.md) has all of it.

The app shows the Admin page when [`/api/login`](auth.md#post-apilogin-required-when-auth-is-true) (or [`/api/me`](auth.md#get-apime-optional)) says `"admin": true`. Hiding it is only a convenience, so every endpoint here must check permissions itself.

## Rules for every request

- Every request carries `Authorization: Bearer <token>` and `Accept: application/json`.
- No token or an invalid one: `401` as JSON, never a redirect to a login page.
- Signed in, but not an admin: `403`.
- Errors may carry `{"error": "…"}` (or Laravel's `{"message": "…"}`). The app shows that text to the admin, so keep it short and readable.
- `404`, `405` or `501` without a message means the server has no admin API. For the endpoints in the other guides, `404`, `405` and `501` always mean "not set up", message or not.

## Endpoints at a glance

| Request                               | Body                                                 | Answer                                                                          |
| ------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------- |
| `GET /api/admin/users`                |                                                      | `[{"name": "user", "admin": false, "sessions": 2, "last_active": 1760400000}]`  |
| `POST /api/admin/users`               | `{"username": "…", "password": "…", "admin": false, "display_name": "…"}` | `201`; `409` or `422` when the name is taken, `422` when a field is missing     |
| `PATCH /api/admin/users/<name>`       | `{"admin": true}`, `{"password": "…"}`, `{"display_name": "…"}` and/or `{"must_change_password": true}` | `200`. A new password ends the user's sessions                                  |
| `DELETE /api/admin/users/<name>`      |                                                      | `200`                                                                           |
| `POST /api/admin/users/<name>/logout` |                                                      | `200`: every session of that user ends                                          |
| `POST /api/admin/rescan`              |                                                      | `{"movies": 12, "series": 3, "episodes": 80}`: read the media folders again now |

`<name>` is the **username**, percent-encoded (`Jo Ann` becomes `Jo%20Ann`), not a numeric id. It's the `name` the user list gives, so in the list `name` must be the username, not the display name. An unknown username gets `404`, ideally with `{"message": "Not found"}`.

## `GET /api/admin/users`

`200` with every user who can sign in:

```json
[
  { "name": "boss", "admin": true, "sessions": 2, "last_active": 1792101466 },
  { "name": "Jo Ann", "admin": false, "sessions": 0, "last_active": null }
]
```

| Field         | Type        | Needed       |                                                                                                   |
| ------------- | ----------- | ------------ | ------------------------------------------------------------------------------------------------- |
| `name`        | string      | **required** | The username, used in the URLs below                                                              |
| `admin`       | bool        |              | Shown as _Admin_ or _User_. Missing counts as `false`                                             |
| `sessions`    | int         |              | How many devices are signed in (active tokens)                                                    |
| `display_name` | string     |              | Shown instead of the username when set; the username goes under it as `@name` |
| `must_change_password` | bool |            | The user is asked to pick a new password (see below). Missing counts as `false` |
| `last_active` | int or null |              | Unix time the user was last active: their last saved watch progress or last token use, whichever is later. `null` if never |

This is also how the app finds out the server has an admin API at all: without it, the Admin page says the server can't be managed from the app.

## `POST /api/admin/users`

Adds a user.

| Field      | Type   | Needed       |                                     |
| ---------- | ------ | ------------ | ----------------------------------- |
| `username` | string | **required** | Also used as the display name       |
| `password` | string | **required** |                                     |
| `admin`    | bool   |              | Defaults to `false`                 |
| `display_name` | string |          | Optional; defaults to the username  |

`201` on success. The body is ignored; returning the new user in the list shape is a good choice. `409` or `422` with a message when the username is taken, `422` when a field is missing or too short (for example `{"message": "That username is already taken"}`). Length rules are up to you; put them in the message.

Users created here must be able to sign in from the app right away. The app has no change-password screen, so don't let a "must change password" flag block `/api/login`.

## `PATCH /api/admin/users/<name>`

Changes a user. The body has any of `admin`, `password`, `display_name` and `must_change_password`:

```json
{ "admin": true }
```

`200` on success; the body is ignored. Setting a new password must also delete all of that user's tokens, so every device they're on is signed out.

`must_change_password` is the user menu's _Require a new password_. While it's `true`, report it from [`GET /api/me`](auth.md#get-apime-optional); the app then asks the user for a new password each time it opens (they can pick _Later_) and sends it to [`POST /api/password`](auth.md#post-apipassword-optional), which clears the flag. `422` with a message when the change isn't allowed, for example `{"message": "You cannot remove your own admin rights"}`.

## `DELETE /api/admin/users/<name>`

Removes a user, along with their tokens and watch progress. `200` on success; the body is ignored. `422` with a message when it isn't allowed, for example `{"message": "You cannot delete your own account"}`.

## `POST /api/admin/users/<name>/logout`

Deletes every token of that user, so all their devices are signed out. `200` on success; the body is ignored (for example `{"message": "Signed out everywhere", "sessions": 2}`).

## `POST /api/admin/rescan`

Reads the media folders again now.

```json
{ "movies": 12, "series": 3, "episodes": 80 }
```

| Field      | Type | Needed |                                                                                          |
| ---------- | ---- | ------ | ---------------------------------------------------------------------------------------- |
| `movies`   | int  |        | Movies in the library after the scan                                                     |
| `series`   | int  |        | Series in the library after the scan                                                     |
| `episodes` | int  |        | Episodes in the library after the scan                                                   |
| `running`  | bool |        | `true` when part of the scan carries on in the background, so the counts are from before it's done |

Leave out any count you can't get, or answer `{}`. When the server answers, the app reloads the library. With `"running": true` it tells the admin that new titles show up once the server finishes and to press _Refresh_ then. A scan that fails gets `500` with a message, for example `{"message": "Scan failed: …"}`.

## Safety

- Refuse to demote or delete yourself with `422` and a message. The app never offers it, but the server must refuse it too.
- Refuse to demote or delete the last admin with `422`. With the rule above this can only happen to the caller, but keep it as a guard.
- Deleting a user also deletes their tokens and watch progress.
- Turn off debug output in production, so error JSON doesn't carry stack traces.

## `GET /admin`: optional

Your own admin web page. When it exists, the app's Admin page has an _Open web admin_ button.

## Laravel example

```php
// routes/api.php
// optional: the app's Admin page
Route::middleware(['auth:sanctum', 'can:admin'])->prefix('admin')->group(function () {
    Route::get('/users', fn () => User::all()->map(fn ($u) => [
        'name' => $u->username, 'admin' => (bool) $u->is_admin, 'sessions' => $u->tokens()->count(),
    ]));
    Route::post('/users', function (Request $r) {
        $d = $r->validate(['username' => 'required|unique:users', 'password' => 'required|min:6', 'admin' => 'boolean'],
            ['username.unique' => 'That username is already taken']);
        User::create(['username' => $d['username'], 'name' => $d['username'],
            'password' => Hash::make($d['password']), 'is_admin' => $d['admin'] ?? false]);
        return response()->json([], 201);
    });
    Route::patch('/users/{user:username}', function (Request $r, User $user) {
        if ($r->has('admin') && !$r->boolean('admin') && $user->is($r->user())) {
            return response()->json(['message' => 'You cannot remove your own admin rights'], 422);
        }
        if ($r->has('admin')) $user->is_admin = $r->boolean('admin');
        if ($r->filled('password')) { $user->password = Hash::make($r->password); $user->tokens()->delete(); }
        $user->save();
        return [];
    });
    Route::delete('/users/{user:username}', function (Request $r, User $user) {
        if ($user->is($r->user())) {
            return response()->json(['message' => 'You cannot delete your own account'], 422);
        }
        $user->tokens()->delete();
        $user->delete();
        return [];
    });
    Route::post('/users/{user:username}/logout', function (User $user) {
        $user->tokens()->delete();
        return [];
    });
    Route::post('/rescan', function () {
        // re-read your media folders, e.g. Artisan::call('library:scan')
        return ['movies' => Movie::count(), 'series' => Series::count(), 'episodes' => Episode::count()];
    });
});
```

For `can:admin`, define the gate in a service provider: `Gate::define('admin', fn ($u) => $u->is_admin);`. Add a check that refuses to demote or delete the last admin. Give `/api/*` JSON `404`s (`{"message": "Not found"}`) in `bootstrap/app.php` so an unknown username isn't answered with an HTML page.

The rest of a Laravel server is under [Examples](examples.md#laravel).

## Testing with curl

With `SERVER` and an admin's `TOKEN` set as in [Testing with curl](testing.md):

```sh
# Admin API present? Expect a JSON list of users.
curl -s -H "Authorization: Bearer $TOKEN" -H 'Accept: application/json' $SERVER/api/admin/users

# Rescan. Expect counts.
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H 'Accept: application/json' $SERVER/api/admin/rescan
```
