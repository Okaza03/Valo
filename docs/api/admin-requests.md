# Requests and the Discord bot

[← Back to the Admin API](admin.md)

The Admin page's _Requests_ section is a queue of movies and series people asked for. Usually they ask in Discord, through a bot. The admin approves, denies or marks each request available in the app, and the bot tells the requester on Discord.

```
 Discord user                 Discord bot                     Your server                     Valo (admin)
 /request Dune  ─────────▶  POST /api/requests  ──────────▶  stores it, status "pending"  ──▶  Requests: Approve / Deny
                                                                                               PATCH /api/admin/requests/<id>
 DM: "Dune was approved" ◀─  GET /api/requests?updated_after  ◀─  status "approved", updated now ◀─┘
```

The server sits in the middle, so the bot and the app never talk to each other directly. That means any bot works, as long as it speaks the two small APIs below, and the requests survive the bot being offline.

**The quickest way:** invite the ready-made Valo bot to your Discord server and point it at your server with `/setup`. It needs only step 1 below and a public HTTPS address; see [The Valo Discord bot](discord-bot.md). To keep everything on your own network instead, run your own bot (steps 2 and 3, or [the Valo bot's own code](discord-bot.md#7-run-your-own-bot-instead)).

You need three things:

1. [The server endpoints](#1-the-server-endpoints): a request queue on your server.
2. [A Discord bot](#2-create-the-discord-bot): an application in Discord's developer portal, invited to your Discord server.
3. [The bot program](#3-run-the-bot): a small script that turns `/request` into API calls. A complete one is below.

With the bundled reference server, step 1 is done: start it with `--bot-token <secret>` (or `VALO_BOT_TOKEN=<secret>`) and skip to step 2.

## 1. The server endpoints

### The request

Every endpoint below sends and returns requests in this shape:

```json
{
  "id": "840c0760",
  "title": "Dune: Part Two",
  "year": 2024,
  "kind": "movie",
  "imdb_id": "tt15239678",
  "poster": "https://m.media-amazon.com/images/…jpg",
  "requested_by": "user",
  "requester_id": "123456789012345678",
  "source": "discord",
  "status": "pending",
  "note": "the 4K one if you can",
  "created": 1792101466,
  "updated": 1792101466
}
```

| Field          | Type           | Needed       |                                                                                                   |
| -------------- | -------------- | ------------ | ------------------------------------------------------------------------------------------------- |
| `id`           | string or int  | **required** | Used in the URLs below                                                                            |
| `title`        | string         | **required** |                                                                                                   |
| `year`         | int            |              |                                                                                                   |
| `kind`         | string         |              | `"movie"` or `"series"`                                                                           |
| `imdb_id`      | string         |              | With it, the app can tell the title is already in the library (when the library sends `imdb_id`)  |
| `poster`       | URL            |              | Shown next to the request                                                                         |
| `requested_by` | string         |              | Display name, shown to the admin                                                                  |
| `requester_id` | string         |              | The Discord user id, so the bot knows whom to tell. Store it as a **string**: Discord ids don't fit in a JavaScript number |
| `source`       | string         |              | `"discord"`, `"app"`, … shown as "via Discord"                                                    |
| `status`       | string         | **required** | `"pending"`, `"approved"`, `"available"` or `"denied"`                                            |
| `note`         | string or null |              | What the requester wrote; when the admin denies a request, the reason replaces it                 |
| `created`      | int            |              | Unix time it was asked for                                                                        |
| `updated`      | int            | **required** for the bot | Unix time of the last change. The bot uses it to find answers                         |
| `tmdb_id`      | string or int  |              | Kept for the server's own use (the app doesn't show it)                                           |
| `message_id`   | string         |              | The bot's message about it in Discord, so the bot can delete it when the request is answered      |
| `notified`     | bool           |              | `false` when the bot couldn't tell the requester about the last answer (their DMs are closed); the app warns the admin |

Statuses go `pending` → `approved` → `available`, or `pending` → `denied`. An admin can also put one back to `pending`.

### For the bot

The bot signs in with its own secret, not a user's password: a long random string you make up (`openssl rand -hex 32`) and give to both the server and the bot. It's sent as `Authorization: Bearer <secret>`. Compare it in constant time (`hmac.compare_digest`, `hash_equals`).

| Request                              | Body                    | Answer                                                                          |
| ------------------------------------ | ----------------------- | ------------------------------------------------------------------------------- |
| `POST /api/requests`                 | a request, without `id`, `status`, `created`, `updated` | `201` with the stored request; `422` without a `title`. When the same title is already requested and still open, `200` with that request instead of a duplicate, so the bot can answer "already requested" |
| `GET /api/requests?updated_after=T`  |                         | every request whose `updated` is later than `T` (unix time); `0` for all        |
| `GET /api/requests/<id>`             |                         | one request, `404` if unknown                                                   |
| `PATCH /api/requests/<id>`           | `{"notified": false}`, `{"message_id": "…"}` | optional: the bot reports back. May also take `status` and `note` (a bot-side answer) |
| `DELETE /api/requests/<id>`          |                         | optional: for a bot command that clears requests                                |
| `POST /api/log`                      | `{"level": "warning", "message": "…"}` | optional: the bot writes a line to the admin's [live log](admin-dashboard.md#get-apiadminlogs) (a failed DM, a card it couldn't delete) |

The server sets `id`, `created` and `updated`, and `status` to `"pending"`. A bot that finds the title is already on disk may send `"status": "available"`, so the request is kept as asked for and fulfilled at once. After telling (or failing to tell) the requester, the bot can report `notified` with `PATCH /api/requests/<id>` and `{"notified": false}`. A signed-in user's token may use the same endpoints; that's optional. Then `requested_by` is their username and `source` is `"app"`. `GET /api/requests` gives them everyone's requests, so the app can show *Requested by alice on 23 Oct 2026* instead of a Request button, and nobody asks for a title twice. Other people's requests leave out `requester_id`, `note`, `message_id` and `notified`. A server that only sends a user their own requests still works, but then two people can ask for the same title.

"Already requested" means an open (`pending` or `approved`) request with the same `imdb_id`, or, without one, the same title, year and kind. A `denied` request doesn't count, so someone else can ask again.

A bot's own `PATCH` of `notified` or `message_id` shouldn't change `updated`, or the bot sees its own change as a new answer on the next poll.

Remember when the bot last called: [`/api/admin/status`](admin-dashboard.md#get-apiadminstatus) can then report it on the Dashboard as `{"name": "Discord bot", "ok": true, "detail": "last seen 40 s ago"}`.

### For the app

Admins only, under the [usual rules](admin.md#rules-for-every-request).

| Request                              | Body                                       | Answer                                                    |
| ------------------------------------ | ------------------------------------------ | --------------------------------------------------------- |
| `GET /api/admin/requests`            |                                            | every request, newest first                               |
| `PATCH /api/admin/requests/<id>`     | `{"status": "denied", "note": "No 4K yet"}` | `200`; set `updated` to now. `note` is optional           |
| `DELETE /api/admin/requests/<id>`    |                                            | `200`. The requester isn't told                           |

The app checks for new requests every 30 seconds while the Admin page is open and shows how many are pending in the sidebar.

## 2. Create the Discord bot

1. Open the [Discord Developer Portal](https://discord.com/developers/applications) and sign in.
2. **New Application**, name it (for example _Valo Requests_), accept the terms, **Create**.
3. Optional: on **General Information**, give it an icon and a description.
4. Open **Bot** in the left menu.
   - **Reset Token**, confirm, and **Copy**. This is the bot's password (`DISCORD_TOKEN` below). Keep it secret; if it leaks, reset it here.
   - Leave the privileged intents (Presence, Server Members, Message Content) **off**. Slash commands don't need them.
   - Turn **Public Bot** off if only you should be able to invite it.
5. Open **OAuth2** → **URL Generator**.
   - Scopes: **bot** and **applications.commands**.
   - Bot permissions: **Send Messages**, **Embed Links**, **Read Message History**.
   - Copy the generated URL at the bottom, open it in a browser, pick your Discord server and **Authorize**. You need _Manage Server_ there.
6. Optional, for a channel where new requests are announced to the admins: in Discord, turn on **User Settings → Advanced → Developer Mode**, then right-click the channel → **Copy Channel ID** (`NOTIFY_CHANNEL_ID` below).

## 3. Run the bot

This bot (Python 3.9+, [discord.py](https://discordpy.readthedocs.io/) 2.x) adds three commands:

- `/request title [kind] [year] [note]`: asks for a title. With an OMDb key it finds the exact title, year, poster and IMDb id first.
- `/myrequests`: the caller's requests and their status.
- It DMs the requester when the admin answers, and posts new requests to `NOTIFY_CHANNEL_ID` if set.

Save it as `valo_bot.py`:

```python
"""Valo request bot: /request in Discord -> the Valo server's request queue."""
import json
import os
import time
from pathlib import Path

import aiohttp
import discord
from discord import app_commands
from discord.ext import tasks

DISCORD_TOKEN = os.environ["DISCORD_TOKEN"]
VALO_URL = os.environ["VALO_URL"].rstrip("/")          # e.g. http://192.168.1.20:8080
VALO_BOT_TOKEN = os.environ["VALO_BOT_TOKEN"]          # the shared secret
OMDB_KEY = os.environ.get("OMDB_API_KEY")              # optional, http://www.omdbapi.com/apikey.aspx
NOTIFY_CHANNEL_ID = int(os.environ.get("NOTIFY_CHANNEL_ID") or 0)
STATE = Path(os.environ.get("BOT_STATE", "valo_bot_state.json"))

HEADERS = {"Authorization": f"Bearer {VALO_BOT_TOKEN}", "Accept": "application/json"}
ANSWERS = {
    "approved": "was approved and will be added soon",
    "available": "is now available on Valo, enjoy!",
    "denied": "was denied",
}


def load_state():
    try:
        return json.loads(STATE.read_text())
    except (OSError, ValueError):
        return {"since": int(time.time()), "seen": {}}


class Bot(discord.Client):
    def __init__(self):
        super().__init__(intents=discord.Intents.default())
        self.tree = app_commands.CommandTree(self)
        self.state = load_state()
        self.http_session = None

    async def setup_hook(self):
        self.http_session = aiohttp.ClientSession(headers=HEADERS)
        await self.tree.sync()  # registers the slash commands (can take a minute the first time)
        self.poll.start()

    async def api(self, method, path, **kw):
        async with self.http_session.request(method, VALO_URL + path, **kw) as r:
            body = await r.json(content_type=None)
            if r.status >= 400:
                raise RuntimeError(body.get("error") or body.get("message") or f"server answered {r.status}")
            return body

    async def omdb(self, title, kind, year):
        if not OMDB_KEY:
            return {}
        q = {"apikey": OMDB_KEY, "t": title, "type": "series" if kind == "series" else "movie"}
        if year:
            q["y"] = str(year)
        async with aiohttp.ClientSession() as s, s.get("https://www.omdbapi.com/", params=q) as r:
            d = await r.json(content_type=None)
        if d.get("Response") != "True":
            return {}
        found = {"title": d["Title"], "imdb_id": d.get("imdbID")}
        if d.get("Year", "")[:4].isdigit():
            found["year"] = int(d["Year"][:4])
        if d.get("Poster", "N/A") != "N/A":
            found["poster"] = d["Poster"]
        return found

    @tasks.loop(seconds=60)
    async def poll(self):
        """Tell requesters about answers, and admins about new requests."""
        try:
            changed = await self.api("GET", f"/api/requests?updated_after={self.state['since'] - 5}")
        except Exception as e:  # server down: try again next minute
            print("poll failed:", e)
            return
        for r in changed:
            rid, status = str(r["id"]), r.get("status", "pending")
            if self.state["seen"].get(rid) == status:
                continue
            first_time = rid not in self.state["seen"]
            self.state["seen"][rid] = status
            self.state["since"] = max(self.state["since"], r.get("updated") or 0)
            if first_time and status == "pending":
                await self.announce(r)
            elif status in ANSWERS and r.get("requester_id"):
                await self.tell(r, status)
        STATE.write_text(json.dumps(self.state))

    async def announce(self, r):
        if not NOTIFY_CHANNEL_ID:
            return
        channel = self.get_channel(NOTIFY_CHANNEL_ID)
        if channel:
            year = f" ({r['year']})" if r.get("year") else ""
            await channel.send(f"New request: **{r['title']}**{year} from {r.get('requested_by') or 'someone'}. "
                               "Answer it on Valo's Admin page.")

    async def tell(self, r, status):
        try:
            user = await self.fetch_user(int(r["requester_id"]))
            year = f" ({r['year']})" if r.get("year") else ""
            text = f"Your request for **{r['title']}**{year} {ANSWERS[status]}."
            if status == "denied" and r.get("note"):
                text += f"\nReason: {r['note']}"
            await user.send(text)
        except discord.HTTPException as e:  # DMs closed, unknown user, …
            print(f"couldn't DM {r.get('requested_by')}: {e}")


bot = Bot()


@bot.tree.command(description="Ask for a movie or series on Valo")
@app_commands.describe(title="What to add", kind="Movie or series", year="Release year, if there are several", note="Anything the admin should know")
@app_commands.choices(kind=[app_commands.Choice(name="Movie", value="movie"), app_commands.Choice(name="Series", value="series")])
async def request(inter: discord.Interaction, title: str, kind: app_commands.Choice[str] = None,
                  year: int = None, note: str = None):
    await inter.response.defer(ephemeral=True)
    k = kind.value if kind else "movie"
    body = {"title": title, "year": year, "kind": k, "note": note, "source": "discord",
            "requested_by": inter.user.display_name, "requester_id": str(inter.user.id)}
    body.update(await bot.omdb(title, k, year))
    try:
        r = await bot.api("POST", "/api/requests", json=body)
    except Exception as e:
        return await inter.followup.send(f"Couldn't send the request: {e}", ephemeral=True)
    bot.state["seen"][str(r["id"])] = "pending"
    await bot.announce(r)
    embed = discord.Embed(title=r["title"] + (f" ({r['year']})" if r.get("year") else ""),
                          description="Request sent. I'll message you when it's answered.")
    if r.get("poster"):
        embed.set_thumbnail(url=r["poster"])
    await inter.followup.send(embed=embed, ephemeral=True)


@bot.tree.command(description="Your Valo requests and their status")
async def myrequests(inter: discord.Interaction):
    await inter.response.defer(ephemeral=True)
    try:
        everything = await bot.api("GET", "/api/requests?updated_after=0")
    except Exception as e:
        return await inter.followup.send(f"Couldn't reach Valo: {e}", ephemeral=True)
    mine = [r for r in everything if r.get("requester_id") == str(inter.user.id)]
    if not mine:
        return await inter.followup.send("You haven't requested anything yet. Try /request.", ephemeral=True)
    lines = [f"**{r['title']}** - {r.get('status', 'pending')}" for r in mine[-15:]]
    await inter.followup.send("\n".join(lines), ephemeral=True)


bot.run(DISCORD_TOKEN)
```

Install and start it on any always-on machine that can reach the server (the server itself is fine):

```sh
python3 -m venv ~/valo-bot && ~/valo-bot/bin/pip install "discord.py>=2.3"
export DISCORD_TOKEN=…            # from step 2
export VALO_URL=http://192.168.1.20:8080
export VALO_BOT_TOKEN=…           # the same secret the server has
export NOTIFY_CHANNEL_ID=…        # optional
export OMDB_API_KEY=…             # optional
~/valo-bot/bin/python valo_bot.py
```

Type `/request` in your Discord server. The first time, Discord can take a minute to show the commands. The request shows up in the app under _Admin → Requests_.

To keep it running, a systemd service (`/etc/systemd/system/valo-bot.service`):

```ini
[Unit]
Description=Valo request bot
After=network-online.target

[Service]
User=valo
WorkingDirectory=/home/valo
EnvironmentFile=/home/valo/valo-bot.env
ExecStart=/home/valo/valo-bot/bin/python /home/valo/valo_bot.py
Restart=always

[Install]
WantedBy=multi-user.target
```

Put the `KEY=value` lines in `valo-bot.env`, then `sudo systemctl enable --now valo-bot`.

### Using another bot

Any bot works if it does what the script above does: `POST /api/requests` with the bot secret, and `GET /api/requests?updated_after=…` now and then to see answers. With a bot that keeps its own database (Requestrr, Doplarr, a bot you already run), add a sync job on your server that copies its requests into this queue and writes the admin's answers back.

## Testing with curl

```sh
# the bot asks for something
curl -s -X POST -H "Authorization: Bearer $BOT_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"Dune","year":2021,"kind":"movie","requested_by":"user","requester_id":"123"}' $SERVER/api/requests

# the admin sees it, and answers
curl -s -H "Authorization: Bearer $TOKEN" $SERVER/api/admin/requests
curl -s -X PATCH -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"status":"approved"}' $SERVER/api/admin/requests/<id>

# the bot finds the answer
curl -s -H "Authorization: Bearer $BOT_TOKEN" "$SERVER/api/requests?updated_after=0"
```

## Troubleshooting

| Problem | Fix |
| ------- | --- |
| _Requests isn't set up_ in the app | `GET /api/admin/requests` answers `404`. Add the endpoints in step 1, or start the reference server with `--bot-token` |
| `/request` doesn't show in Discord | Invite the bot with the **applications.commands** scope (step 5), wait a minute, restart Discord |
| "Couldn't send the request: sign in required" | The bot's `VALO_BOT_TOKEN` doesn't match the server's |
| Requesters get no DM | They have DMs from server members turned off. Set `NOTIFY_CHANNEL_ID` and mention them there instead |
| The Dashboard says the Discord bot is down | The server hasn't heard from the bot in 10 minutes. Check that it runs and can reach `VALO_URL` |
