# Layout

[← Back to the Admin API](admin.md)

Admins choose which rows everyone sees on the home page, and in what order, from the Admin page's _Layout_ section. Without these endpoints, that section says _Not set up_ and links here, and the home page shows every row.

There are two parts:

1. **`GET /api/layout`** gives the layout to every signed-in user. The app loads it with the library.
2. **`GET` and `PATCH /api/admin/layout`** let admins see and change it.

## Row keys

`home_hidden` lists the rows nobody sees. Anything else shows (when it has something in it).

| Key                                | Row                                                                      |
| ---------------------------------- | ------------------------------------------------------------------------ |
| `featured`                         | The featured banner at the top                                           |
| `genre_tiles`                      | The genre tiles under it, in the styles that have them                   |
| `continue`                         | Continue watching                                                        |
| `mylist`                           | My list                                                                  |
| `recent`                           | Recently added                                                           |
| `top`                              | Top rated (or Top 10)                                                    |
| `movies`                           | Movies                                                                   |
| `series`                           | TV shows                                                                 |
| `genres`                           | Every genre row                                                          |
| `genre:<name>`                     | One genre's row and tile: `genre:Drama`                                  |
| `catalog:<addon id>:<type>:<id>`   | An [addon](admin-addons.md) catalog's row: `catalog:ba3e09ce:movie:top`  |

The server should keep keys it doesn't know, so newer apps can add more.

## Row order

`home_order` lists the rows top to bottom, by the same keys. The featured banner and genre tiles always stay at the top, so only `continue`, `mylist`, `recent`, `top`, `movies`, `series`, `genres` and the `catalog:…` keys count here. Empty means the usual order: the first four, then the addon catalogs, then the last three. A row missing from the list, like a newly added addon's catalog, goes right after the row it usually follows.

## `GET /api/layout`

Any signed-in user. `200`:

```json
{
  "home_hidden": ["recent", "genre:Drama", "catalog:ba3e09ce:movie:top"],
  "home_order": ["mylist", "continue", "catalog:ba3e09ce:movie:top", "recent", "top", "movies", "series", "genres"]
}
```

A server without this endpoint (`404`) has nothing hidden, in the usual order.

## `GET /api/admin/layout`

Admins only. The same as `GET /api/layout`.

## `PATCH /api/admin/layout`

Admins only. Replaces the fields it sends, and answers `200` with the whole layout.

```json
{ "home_hidden": ["recent", "genre:Drama"], "home_order": ["mylist", "continue"] }
```

`400` for a body that isn't a JSON object. The reference server keeps it in `.valo/layout.json`.
