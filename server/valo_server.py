#!/usr/bin/env python3
"""Minimal reference media server for Valo (Python 3.9+, standard library only).

    python3 valo_server.py ~/Videos                  # open library, port 8080
    python3 valo_server.py ~/Videos --add-user user  # enable sign-in

Expected layout (anything else is ignored):

    Movies/Inception (2010)/Inception.mkv   (+ poster.jpg, fanart.jpg, info.json)
    Series/The Office/Season 1/The.Office.S01E01.Pilot.mkv   (+ poster.jpg, fanart.jpg, info.json)

Subtitles (.srt, .vtt, .ass, .ssa) go in a Subs folder in the movie's or the
series' folder (or a season folder): Subs/English.srt for a movie; for an
episode, put its code in the name (Subs/S01E01.en.srt) or use a folder named
like the video (Subs/The.Office.S01E01.Pilot/2_English.srt). Files next to the
video named after it (Inception.en.srt) work too.

info.json may set title, year, overview, genres, rating, duration, and the
details-page extras: director, creators, cast ([{"name", "character", "photo"}],
photo being a URL or an image file next to info.json), released, end_year,
network, status, budget, revenue, score, score_source, reviews. A series can add
"episodes": {"S01E01": {"title", "overview", "aired", "score"}}. A logo.png
next to the video is used as title artwork.

With sign-in on, each user's watch progress is kept in .valo/progress.json
and served from /api/progress, so it follows them between devices, and
/api/sessions lists the devices they're signed in on. Admins
(--add-user NAME --admin) get the app's Admin page through /api/admin/...:
users, a dashboard with syncs and the live log, analytics from watch
progress, requests (filled by a Discord bot, see docs/api/admin-requests.md),
editing titles (written to info.json; IMDb lookups need --omdb-key),
collections (.valo/collections.json) and Stremio addons (.valo/addons.json):
their catalogs, metadata and subtitles show up for every user.
"""

import argparse
import collections
import datetime
import getpass
import glob
import hashlib
import hmac
import json
import mimetypes
import os
import re
import secrets
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, quote, unquote, urlencode, urlparse

VERSION = "1.1"
STARTED = time.time()


class Log:
    """The last lines of the server's log, for the app's live log feed."""

    def __init__(self, keep=1000):
        self.lines = collections.deque(maxlen=keep)
        self.next = 1
        self.lock = threading.Lock()

    def __call__(self, message, level="info"):
        with self.lock:
            self.lines.append(
                {"id": self.next, "time": int(time.time()), "level": level, "message": message}
            )
            self.next += 1
        print(f"[{level}] {message}", flush=True)

    def after(self, after):
        with self.lock:
            lines = [l for l in self.lines if l["id"] > after]
        return lines if after else lines[-200:]


log = Log()

VIDEO = {
    ".mp4",
    ".mkv",
    ".webm",
    ".avi",
    ".mov",
    ".m4v",
    ".flv",
    ".wmv",
    ".mpg",
    ".mpeg",
    ".ogv",
    ".ts",
    ".3gp",
}
IMAGE = (".jpg", ".jpeg", ".png", ".webp")
MOVIE_DIRS = {"movies", "films"}
SERIES_DIRS = {"series", "tv", "tv shows", "shows"}
EPISODE = re.compile(
    r"(?i)(?:^|[^a-z0-9])s(\d{1,2})[ ._-]?e(\d{1,3})(.*)$|(?:^|\D)(\d{1,2})x(\d{2,3})(.*)$"
)
JUNK = re.compile(
    r"(?i)\b(19\d\d|20\d\d|2160p|1080p|720p|480p|4k|bluray|webrip|web-dl|hdtv|x26[45]|h26[45]|hevc)\b.*$"
)


def clean(name):
    name = re.sub(r"\[.*?\]", "", name)
    m = re.search(r"\b(19\d\d|20\d\d)\b", name[1:])
    year = int(m.group(1)) if m else None
    return re.sub(r"[._()]+", " ", name[:1] + JUNK.sub("", name[1:])).strip(" -"), year


SUB_EXTS = {".srt": "srt", ".vtt": "vtt", ".ass": "ass", ".ssa": "ssa"}
LANGUAGES = {
    "en": "English", "nb": "Norwegian", "sv": "Swedish",
    "da": "Danish", "fi": "Finnish", "is": "Icelandic", "de": "German",
    "nl": "Dutch", "fr": "French", "es": "Spanish", "pt": "Portuguese",
    "it": "Italian", "pl": "Polish", "cs": "Czech", "sk": "Slovak",
    "hu": "Hungarian", "ro": "Romanian", "bg": "Bulgarian", "el": "Greek",
    "tr": "Turkish", "ru": "Russian", "uk": "Ukrainian", "hr": "Croatian",
    "sr": "Serbian", "sl": "Slovenian", "et": "Estonian", "lv": "Latvian",
    "lt": "Lithuanian", "ar": "Arabic", "he": "Hebrew", "fa": "Persian",
    "hi": "Hindi", "th": "Thai", "vi": "Vietnamese", "id": "Indonesian",
    "ms": "Malay", "zh": "Chinese", "ja": "Japanese", "ko": "Korean",
}
# three-letter codes and other spellings seen in file names
LANGUAGE_ALIASES = {
    "eng": "en", "no": "nb", "nor": "nb", "nob": "nb", "norsk": "nb", "bokmal": "nb", "swe": "sv",
    "dan": "da", "fin": "fi", "ger": "de", "deu": "de", "dut": "nl", "nld": "nl",
    "fre": "fr", "fra": "fr", "spa": "es", "por": "pt", "ita": "it", "pol": "pl",
    "cze": "cs", "ces": "cs", "hun": "hu", "rum": "ro", "ron": "ro", "gre": "el",
    "ell": "el", "tur": "tr", "rus": "ru", "ukr": "uk", "ara": "ar", "heb": "he",
    "chi": "zh", "zho": "zh", "jpn": "ja", "kor": "ko", "brazilian": "pt",
    "chs": "zh", "cht": "zh", "simplified": "zh", "traditional": "zh",
}
LANGUAGE_NAMES = {v.lower(): k for k, v in LANGUAGES.items()}


def sub_language(stem):
    """("en", forced, sdh) from a subtitle's file name."""
    words = [w for w in re.split(r"[^a-z]+", stem.lower()) if w]
    forced = "forced" in words
    sdh = "sdh" in words or "hi" in words or "cc" in words
    for w in reversed(words):
        if w in LANGUAGE_NAMES:
            return LANGUAGE_NAMES[w], forced, sdh
        if w in LANGUAGE_ALIASES:
            return LANGUAGE_ALIASES[w], forced, sdh
    # two-letter codes only as their own part ("Movie.en.srt", "en.srt"), where
    # they can't be part of a word; "hi" is also SDH, so it never means Hindi here
    for w in reversed(words):
        if len(w) == 2 and w in LANGUAGES and w != "hi":
            return w, forced, sdh
    return "und", forced, sdh


def subs_dirs(folder):
    """The Subs folders in `folder` (any letter case)."""
    try:
        return [d for d in folder.iterdir() if d.is_dir() and d.name.lower() == "subs"]
    except OSError:
        return []


def subtitles(lib, video, places, episode=None):
    """The subtitle list for a video: files under the Subs folders in `places`
    (for an episode, only the ones naming it) and files named after the video."""
    files = []
    for folder in places:
        for d in subs_dirs(folder):
            for f in sorted(d.rglob("*")):
                if f.suffix.lower() not in SUB_EXTS or not f.is_file():
                    continue
                if episode is not None:
                    owner = f.parent.name if f.parent != d else ""
                    hit = owner == video.stem
                    for name in (f.stem, owner):
                        m = EPISODE.search(name)
                        if m:
                            got = (m.group(1), m.group(2)) if m.group(1) else (m.group(4), m.group(5))
                            hit = hit or tuple(int(x) for x in got) == episode
                    if not hit:
                        continue
                files.append(f)
    for f in sorted(video.parent.glob(glob.escape(video.stem) + ".*")):
        if f.suffix.lower() in SUB_EXTS and f.is_file() and f not in files:
            files.append(f)
    out, seen = [], collections.Counter()
    for f in files:
        rest = f.stem[len(video.stem) :] if f.stem.startswith(video.stem) else f.stem
        lang, forced, sdh = sub_language(rest)
        label = LANGUAGES.get(lang, "Unknown")
        if sdh:
            label += " (SDH)"
        elif forced:
            label += " (Forced)"
        seen[label] += 1
        if seen[label] > 1:
            label += f" {seen[label]}"
        out.append(
            {
                "id": hashlib.sha1(str(f.relative_to(lib.root)).encode()).hexdigest()[:12],
                "language": lang,
                "label": label,
                "format": SUB_EXTS[f.suffix.lower()],
                "forced": forced,
                "sdh": sdh,
                "url": lib.url("media", f),
            }
        )
    return out



def find_art(folder, names):
    for n in names:
        for ext in IMAGE:
            if (folder / f"{n}{ext}").is_file():
                return folder / f"{n}{ext}"
    return None


QUALITY = re.compile(r"(?i)\b(2160p|4k|1440p|1080p|720p|576p|480p)\b")
EXTRAS = (
    "director",
    "creators",
    "released",
    "end_year",
    "network",
    "status",
    "budget",
    "revenue",
    "score",
    "score_source",
    "reviews",
)


def file_info(lib, f):
    q = QUALITY.search(f.stem)
    return {
        "quality": q and q.group(1).lower().replace("4k", "2160p"),
        "container": f.suffix.lstrip(".").lower(),
        "size": f.stat().st_size,
        "name": f.name,
        "path": "/" + str(f.parent.relative_to(lib.root)),
    }


def extras(lib, info, folder):
    out = {k: info[k] for k in EXTRAS if k in info}
    cast = []
    for c in info.get("cast", []):
        c = dict(c)
        photo = c.get("photo")
        if photo and "://" not in photo and (folder / photo).is_file():
            c["photo"] = lib.url("art", folder / photo)
        cast.append(c)
    if cast:
        out["cast"] = cast
    logo = find_art(folder, ["logo", "clearlogo"])
    if logo:
        out["logo"] = lib.url("art", logo)
    return out


def read_info(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return {}


def write_info(path, fields):
    """Merge `fields` into an info.json; a None value removes the key."""
    info = read_info(path)
    for k, v in fields.items():
        if v is None:
            info.pop(k, None)
        else:
            info[k] = v
    path.write_text(json.dumps(info, indent=2, ensure_ascii=False))


class Library:
    def __init__(self, root, collections=None):
        self.root = root
        self.collections = collections
        self.skipped = []  # video files the scan couldn't place
        self.cache, self.stamp, self.lock = None, 0, threading.Lock()
        # the library as of the last sync: what the syncs compare against,
        # since the cache rescans on its own every minute
        self.synced = None

    def url(self, kind, path):
        return f"/{kind}/" + quote(str(path.relative_to(self.root)))

    def get(self, fresh=False):
        with self.lock:
            if fresh or self.cache is None or time.time() - self.stamp > 60:
                self.cache, self.stamp = self.scan(), time.time()
                if self.synced is None:
                    self.synced = self.cache
            return self.cache

    def category(self, names):
        return [
            d for d in self.root.iterdir() if d.is_dir() and d.name.lower() in names
        ]

    def item_files(self, kind, id):
        """(info.json path, art folder, art name prefix or None) for a library id."""
        rel = id.split(":", 1)[1] if ":" in id else id
        if kind == "movies":
            f = (self.root / rel).resolve()
            if self.root not in f.parents or not f.is_file():
                return None
            top = next(d for d in self.root.iterdir() if d in f.parents)
            own = f.parent != top
            info = f.with_suffix(".json")
            if own and not info.exists():
                info = f.parent / "info.json"
            return info, f.parent, (None if own else f.stem)
        for top in self.category(SERIES_DIRS):
            show = top / rel
            if show.is_dir() and show.parent == top:
                return show / "info.json", show, None
        return None

    def scan(self):
        movies, series = [], []
        self.skipped = []
        for top in self.category(MOVIE_DIRS):
            for f in sorted(top.rglob("*")):
                if f.suffix.lower() not in VIDEO:
                    continue
                own = f.parent != top
                info = read_info(f.with_suffix(".json"))
                if own and not info:
                    info = read_info(f.parent / "info.json")
                title, year = clean(f.parent.name if own else f.stem)
                folder = f.parent
                poster = find_art(
                    folder,
                    [f.stem, f"{f.stem}-poster"]
                    + (["poster", "folder", "cover"] if own else []),
                )
                backdrop = find_art(
                    folder,
                    [f"{f.stem}-fanart"]
                    + (["fanart", "backdrop", "background"] if own else []),
                )
                thumb = find_art(
                    folder, [f"{f.stem}-thumb"] + (["thumb", "landscape"] if own else [])
                )
                movies.append(
                    {
                        "id": "m:" + str(f.relative_to(self.root)),
                        "title": info.get("title", title),
                        "year": info.get("year", year),
                        "overview": info.get("overview", ""),
                        "genres": info.get("genres", []),
                        "rating": info.get("rating"),
                        "duration": info.get("duration"),
                        "added": int(f.stat().st_mtime),
                        "poster": poster and self.url("art", poster),
                        "backdrop": backdrop and self.url("art", backdrop),
                        "thumb": thumb and self.url("art", thumb),
                        "stream": self.url("media", f),
                        "subtitles": subtitles(self, f, [folder] if own else []),
                        "imdb_id": info.get("imdb_id"),
                        "file": file_info(self, f),
                        **(extras(self, info, folder) if own else {}),
                    }
                )
        for top in self.category(SERIES_DIRS):
            for show in sorted(d for d in top.iterdir() if d.is_dir()):
                info = read_info(show / "info.json")
                eps_info = info.get("episodes", {})
                seasons = {}
                for f in sorted(show.rglob("*")):
                    if f.suffix.lower() not in VIDEO:
                        continue
                    m = EPISODE.search(f.stem)
                    if not m:
                        self.skipped.append(str(f.relative_to(self.root)))
                        continue
                    s, e, rest = (
                        (m.group(1), m.group(2), m.group(3))
                        if m.group(1)
                        else (m.group(4), m.group(5), m.group(6))
                    )
                    s, e = int(s), int(e)
                    meta = eps_info.get(f"S{s:02d}E{e:02d}", {})
                    still = find_art(f.parent, [f.stem, f"{f.stem}-thumb"])
                    seasons.setdefault(s, []).append(
                        {
                            "id": "e:" + str(f.relative_to(self.root)),
                            "number": e,
                            "title": meta.get("title")
                            or clean(rest)[0]
                            or f"Episode {e}",
                            "overview": meta.get("overview", ""),
                            "duration": meta.get("duration"),
                            "still": still and self.url("art", still),
                            "stream": self.url("media", f),
                            "subtitles": subtitles(
                                self,
                                f,
                                [show] + [p for p in f.parents if show in p.parents],
                                (s, e),
                            ),
                            "aired": meta.get("aired"),
                            "score": meta.get("score"),
                            "file": file_info(self, f),
                        }
                    )
                if not seasons:
                    continue
                title, year = clean(show.name)
                poster = find_art(show, ["poster", "folder", "cover"])
                backdrop = find_art(show, ["fanart", "backdrop", "background"])
                thumb = find_art(show, ["thumb", "landscape"])
                newest = max(
                    p.stat().st_mtime
                    for p in show.rglob("*")
                    if p.suffix.lower() in VIDEO
                )
                series.append(
                    {
                        "id": "s:" + show.name,
                        "title": info.get("title", title),
                        "year": info.get("year", year),
                        "overview": info.get("overview", ""),
                        "genres": info.get("genres", []),
                        "rating": info.get("rating"),
                        "added": int(newest),
                        "poster": poster and self.url("art", poster),
                        "backdrop": backdrop and self.url("art", backdrop),
                        "thumb": thumb and self.url("art", thumb),
                        "seasons": [
                            {"number": n, "episodes": eps}
                            for n, eps in sorted(seasons.items())
                        ],
                        **extras(self, info, show),
                    }
                )
        if self.collections:
            self.collections.apply(movies, series)
        return {"movies": movies, "series": series}

    def files(self, data, kind):
        """Every video file of `kind` ("movies"/"series") as a relative path."""
        if kind == "movies":
            return {m["id"][2:] for m in data["movies"]}
        return {
            e["id"][2:] for s in data["series"] for se in s["seasons"] for e in se["episodes"]
        }

    def sync(self, what, apply=False):
        """Compare the folders with the cached library; apply for movies/series."""
        with self.lock:
            old = self.synced or self.cache or self.scan()
            fresh = self.scan()
            kinds = ["movies", "series"] if what == "folders" else [what]
            added, removed = [], []
            for k in kinds:
                a, b = self.files(old, k), self.files(fresh, k)
                added += sorted(b - a)
                removed += sorted(a - b)
            if what == "folders" and not apply:
                return {"added": added, "removed": removed, "applied": False}
            if what == "folders":
                self.cache = self.synced = fresh
            else:
                self.cache = dict(self.cache or fresh, **{what: fresh[what]})
                self.synced = dict(old, **{what: fresh[what]})
            self.stamp = time.time()
            return {"added": added, "removed": removed, "applied": True}


class Collections:
    """collections.json: [{"id", "name", "items": [{"type": "movie"|"series", "id"}]}]."""

    def __init__(self, folder):
        self.path = folder / "collections.json"
        data = read_info(self.path)
        self.items = data if isinstance(data, list) else []
        self.lock = threading.Lock()

    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.items, indent=2, ensure_ascii=False))

    def find(self, id):
        return next((c for c in self.items if c["id"] == id), None)

    def apply(self, movies, series):
        by_id = {("movie", m["id"]): m for m in movies}
        by_id.update({("series", s["id"]): s for s in series})
        for c in self.items:
            for n, it in enumerate(c["items"]):
                item = by_id.get((it["type"], it["id"]))
                if item:
                    item["collection"], item["collection_index"] = c["name"], n

    @staticmethod
    def clean(req):
        name = str(req.get("name", "")).strip()
        items = [
            {"type": str(i.get("type")), "id": str(i.get("id"))}
            for i in req.get("items") or []
            if isinstance(i, dict) and i.get("type") in ("movie", "series")
        ]
        # older clients: lists of ids by kind
        items += [{"type": "movie", "id": str(i)} for i in req.get("movies") or []]
        items += [{"type": "series", "id": str(i)} for i in req.get("series") or []]
        return name, items

    def save_one(self, id, req):
        name, items = self.clean(req)
        with self.lock:
            c = self.find(id) if id else None
            if id and not c:
                return None
            if not c:
                c = {"id": secrets.token_hex(4)}
                self.items.append(c)
            if name:
                c["name"] = name
            if "items" in req or "movies" in req or "series" in req or "items" not in c:
                c["items"] = items
            # a title is in one collection at a time
            mine = {(i["type"], i["id"]) for i in c["items"]}
            for other in self.items:
                if other is not c:
                    other["items"] = [
                        i for i in other["items"] if (i["type"], i["id"]) not in mine
                    ]
            self.save()
            return c

    def delete(self, id):
        with self.lock:
            before = len(self.items)
            self.items = [c for c in self.items if c["id"] != id]
            self.save()
            return len(self.items) < before


class Requests:
    """requests.json: what people asked for, e.g. through the Discord bot."""

    STATUSES = ("pending", "approved", "available", "denied")

    def __init__(self, folder):
        self.path = folder / "requests.json"
        data = read_info(self.path)
        self.items = data if isinstance(data, list) else []
        self.lock = threading.Lock()
        self.bot_seen = None  # last time the bot called

    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.items, indent=2, ensure_ascii=False))

    def find(self, id):
        return next((r for r in self.items if r["id"] == id), None)

    def add(self, req, by=None):
        title = str(req.get("title", "")).strip()
        if not title:
            return None
        now = int(time.time())
        r = {
            "id": secrets.token_hex(4),
            "title": title[:200],
            "year": req.get("year") if isinstance(req.get("year"), int) else None,
            "kind": req.get("kind") if req.get("kind") in ("movie", "series") else None,
            "imdb_id": req.get("imdb_id"),
            "poster": req.get("poster"),
            "requested_by": str(req.get("requested_by") or by or "")[:100] or None,
            "requester_id": req.get("requester_id"),
            "source": str(req.get("source") or ("app" if by else "discord")),
            # a bot that found the title already on disk files it as fulfilled
            "status": "available" if req.get("status") == "available" else "pending",
            "note": req.get("note"),
            "tmdb_id": req.get("tmdb_id"),
            "message_id": req.get("message_id"),
            "created": now,
            "updated": now,
        }
        with self.lock:
            self.items.append(r)
            self.save()
        return r

    def update(self, id, status, note):
        with self.lock:
            r = self.find(id)
            if r:
                r["status"], r["updated"] = status, int(time.time())
                r.pop("notified", None)  # the bot reports again for this answer
                if note is not None:
                    r["note"] = note
                self.save()
            return r

    def delete(self, id):
        with self.lock:
            before = len(self.items)
            self.items = [r for r in self.items if r["id"] != id]
            self.save()
            return len(self.items) < before


class AddonError(Exception):
    """An addon the admin can't add; the message says why."""


ADDON_RESOURCES = ("catalog", "meta", "subtitles")


def addon_manifest(url):
    """Fetch a Stremio addon's manifest.json. Catalog, metadata and subtitle
    addons are taken; the videos come from this server, so its streams
    (torrents included) never are."""
    url = url.strip()
    if url.startswith("stremio://"):
        url = "https://" + url[len("stremio://"):]
    if not url.startswith(("http://", "https://")):
        raise AddonError("paste the addon's manifest URL (https://…/manifest.json)")
    if not urlparse(url).path.endswith("/manifest.json"):
        url = url.rstrip("/") + "/manifest.json"
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "valo-server", "Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=20) as r:
            m = json.loads(r.read(1 << 20))
    except (OSError, ValueError) as e:
        raise AddonError(f"couldn't load the addon's manifest: {e}")
    if not isinstance(m, dict) or not m.get("id") or not m.get("name"):
        raise AddonError("that URL isn't a Stremio addon manifest")
    hints = m.get("behaviorHints") or {}
    if hints.get("p2p"):
        raise AddonError("torrent (P2P) addons aren't allowed")
    if hints.get("configurationRequired"):
        raise AddonError("this addon has to be configured first: open its configure page and paste the URL it gives you")
    names = [r if isinstance(r, str) else (r or {}).get("name") for r in m.get("resources") or []]
    if not any(n in ADDON_RESOURCES for n in names):
        if "stream" in names:
            raise AddonError("this is a stream addon; Valo plays from this server, so it doesn't take those")
        raise AddonError("this addon has no catalogs, metadata or subtitles")
    return url, m


def addon_resources(m):
    """The resources Valo uses, each with the types and id prefixes it serves."""
    types = [t for t in m.get("types") or [] if isinstance(t, str)]
    prefixes = m.get("idPrefixes")
    out = []
    for r in m.get("resources") or []:
        r = {"name": r} if isinstance(r, str) else r
        if not isinstance(r, dict) or r.get("name") not in ADDON_RESOURCES:
            continue
        p = r.get("idPrefixes", prefixes)
        out.append(
            {
                "name": r["name"],
                "types": [t for t in r.get("types") or types if isinstance(t, str)],
                "id_prefixes": [x for x in p if isinstance(x, str)] if isinstance(p, list) else None,
            }
        )
    return out


def addon_catalogs(m):
    """Catalogs the app can show as rows: movies and series that need no search or filter."""
    out = []
    for c in m.get("catalogs") or []:
        if not isinstance(c, dict) or c.get("type") not in ("movie", "series") or not c.get("id"):
            continue
        extra = [e for e in c.get("extra") or [] if isinstance(e, dict)]
        if c.get("extraRequired") or any(e.get("isRequired") for e in extra):
            continue
        out.append({"type": c["type"], "id": str(c["id"]), "name": str(c.get("name") or c["id"])[:100]})
    return out


class Addons:
    """addons.json: Stremio addons the admin added (catalogs, metadata,
    subtitles), for every user."""

    def __init__(self, folder):
        self.path = folder / "addons.json"
        data = read_info(self.path)
        self.items = data if isinstance(data, list) else []
        self.lock = threading.Lock()

    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.items, indent=2, ensure_ascii=False))

    def add(self, url, by):
        url, m = addon_manifest(url)
        hints = m.get("behaviorHints") or {}
        with self.lock:
            if any(a["url"] == url for a in self.items):
                raise AddonError(f"{m['name']} is already added")
            a = {
                "id": secrets.token_hex(4),
                "url": url,
                "name": str(m["name"])[:100],
                "description": str(m.get("description") or "")[:500] or None,
                "version": str(m.get("version") or "") or None,
                "logo": m.get("logo") if isinstance(m.get("logo"), str) else None,
                "types": [t for t in m.get("types") or [] if isinstance(t, str)],
                "resources": addon_resources(m),
                "catalogs": addon_catalogs(m),
                "configurable": bool(hints.get("configurable") or hints.get("configurationRequired")),
                "added": int(time.time()),
                "added_by": by,
            }
            self.items.append(a)
            self.save()
        return a

    def delete(self, id):
        with self.lock:
            before = len(self.items)
            self.items = [a for a in self.items if a["id"] != id]
            self.save()
            return len(self.items) < before


def omdb(key, title, year, kind, imdb_id):
    """Look a title up on OMDb (IMDb's data); library-shaped fields."""
    q = {"apikey": key, "plot": "full"}
    if imdb_id:
        q["i"] = imdb_id
    else:
        q.update(t=title, type="series" if kind == "series" else "movie")
        if year:
            q["y"] = year
    url = "https://www.omdbapi.com/?" + urlencode(q)
    with urllib.request.urlopen(url, timeout=20) as r:
        d = json.load(r)
    if d.get("Response") != "True":
        raise LookupError(d.get("Error") or "not found on IMDb")
    na = lambda v: None if v in (None, "", "N/A") else v
    out = {
        "title": na(d.get("Title")),
        "overview": na(d.get("Plot")),
        "genres": [g.strip() for g in (na(d.get("Genre")) or "").split(",") if g.strip()],
        "rating": na(d.get("Rated")),
        "imdb_id": na(d.get("imdbID")),
        "poster": na(d.get("Poster")),
    }
    m = re.match(r"(\d{4})(?:\D+(\d{4}))?", d.get("Year") or "")
    if m:
        out["year"] = int(m.group(1))
        if m.group(2):
            out["end_year"] = int(m.group(2))
    if na(d.get("imdbRating")):
        out["score"], out["score_source"] = float(d["imdbRating"]), "IMDb"
    try:
        out["released"] = datetime.datetime.strptime(d.get("Released", ""), "%d %b %Y").strftime("%Y-%m-%d")
    except ValueError:
        pass
    people = lambda v: [p.strip() for p in (na(v) or "").split(",") if p.strip()]
    if kind == "series":
        out["creators"] = people(d.get("Writer"))
    else:
        out["director"] = na(d.get("Director"))
        out["cast"] = [{"name": n} for n in people(d.get("Actors"))]
    return {k: v for k, v in out.items() if v not in (None, [], "")}


def analytics(progress, lib, days):
    """Plays, hours and top titles from everyone's watch progress."""
    since = time.time() - days * 86400
    data = lib.get()
    titles = {("movie", m["id"]): (m["title"], "movie") for m in data["movies"]}
    for s in data["series"]:
        for se in s["seasons"]:
            for e in se["episodes"]:
                titles[("episode", e["id"])] = (s["title"], "series")
    per_day, top, users = {}, {}, []
    total_plays, total_hours = 0, 0.0
    for user, entries in progress.data.items():
        plays, hours, last = 0, 0.0, None
        for key, e in entries.items():
            if e["updated"] < since:
                continue
            kind, id = key.split(":", 1)
            h = min(e["position"], e["duration"] or e["position"]) / 3600
            day = datetime.date.fromtimestamp(e["updated"]).isoformat()
            d = per_day.setdefault(day, {"day": day, "plays": 0, "hours": 0.0})
            d["plays"] += 1
            d["hours"] += h
            title, tkind = titles.get((kind, id), (None, None))
            if title:
                t = top.setdefault(title, {"title": title, "kind": tkind, "plays": 0, "hours": 0.0})
                t["plays"] += 1
                t["hours"] += h
            plays, hours, last = plays + 1, hours + h, max(last or 0, e["updated"])
        if plays:
            users.append({"name": user, "plays": plays, "hours": round(hours, 1), "last_active": last})
        total_plays, total_hours = total_plays + plays, total_hours + hours
    start = datetime.date.today() - datetime.timedelta(days=days - 1)
    series = []
    for n in range(days):
        day = (start + datetime.timedelta(days=n)).isoformat()
        d = per_day.get(day, {"day": day, "plays": 0, "hours": 0.0})
        series.append({**d, "hours": round(d["hours"], 1)})
    return {
        "plays": total_plays,
        "hours": round(total_hours, 1),
        "active_users": len(users),
        "days": series,
        "top": sorted(top.values(), key=lambda t: -t["plays"])[:20],
        "users": sorted(users, key=lambda u: -u["hours"]),
    }


def user_analytics(progress, users, lib, name, days):
    """One user's watching: totals, when they watch, what and on which devices."""
    since = time.time() - days * 86400
    data = lib.get()
    titles = {("movie", m["id"]): (m, None, None) for m in data["movies"]}
    for s in data["series"]:
        for se in s["seasons"]:
            for e in se["episodes"]:
                label = f"S{se['number']:02}E{e['number']:02} · {e.get('title') or ''}".rstrip(" ·")
                titles[("episode", e["id"])] = (s, e["id"], label)
    per_day, top, genres, recent = {}, {}, {}, []
    hours_of_day, weekdays = [0] * 24, [0] * 7
    plays, hours, movies, episodes, finished, started = 0, 0.0, 0, 0, 0, 0
    for key, e in progress.data.get(name, {}).items():
        if e["updated"] < since:
            continue
        kind, id = key.split(":", 1)
        pos, dur = e["position"], e["duration"]
        h = min(pos, dur or pos) / 3600
        when = datetime.datetime.fromtimestamp(e["updated"])
        day = when.date().isoformat()
        d = per_day.setdefault(day, {"day": day, "plays": 0, "hours": 0.0})
        d["plays"] += 1
        d["hours"] += h
        hours_of_day[when.hour] += 1
        weekdays[when.weekday()] += 1
        plays, hours = plays + 1, hours + h
        done = dur > 0 and pos >= 0.95 * dur
        finished, started = finished + done, started + (not done)
        movies, episodes = movies + (kind == "movie"), episodes + (kind == "episode")
        item, ep, label = titles.get((kind, id), (None, None, None))
        if not item:
            continue
        tkind = "movie" if kind == "movie" else "series"
        t = top.setdefault(item["title"], {"title": item["title"], "kind": tkind, "id": item["id"], "plays": 0, "hours": 0.0})
        t["plays"] += 1
        t["hours"] += h
        for g in item.get("genres") or []:
            genres[g] = genres.get(g, 0.0) + h
        recent.append({
            "type": kind, "id": id, "title": item["title"], "series_id": item["id"] if ep else None,
            "episode": label, "position": pos, "duration": dur, "completed": done, "updated": e["updated"],
        })
    start = datetime.date.today() - datetime.timedelta(days=days - 1)
    series = []
    for n in range(days):
        day = (start + datetime.timedelta(days=n)).isoformat()
        d = per_day.get(day, {"day": day, "plays": 0, "hours": 0.0})
        series.append({**d, "hours": round(d["hours"], 1)})
    info = users.users.get(name, {})
    return {
        "name": name,
        "display_name": info.get("display_name"),
        "admin": bool(info.get("admin")),
        "plays": plays,
        "hours": round(hours, 1),
        "movies": movies,
        "episodes": episodes,
        "finished": finished,
        "in_progress": started,
        "last_active": progress.last_active(name),
        "days": series,
        "hours_of_day": hours_of_day,
        "weekdays": weekdays,
        "top": [{**t, "hours": round(t["hours"], 1)} for t in sorted(top.values(), key=lambda t: -t["hours"])[:20]],
        "genres": [{"name": g, "hours": round(v, 1)} for g, v in sorted(genres.items(), key=lambda g: -g[1])[:10]],
        "recent": sorted(recent, key=lambda r: -r["updated"])[:30],
        "sessions": [
            {k: v for k, v in s.items() if k not in ("id", "current")}
            for s in sorted(users.sessions(name, None), key=lambda s: -(s["last_used"] or 0))
        ],
    }


def download(url, folder, name):
    """Save an image from `url` as folder/name.<ext>, replacing other formats."""
    req = urllib.request.Request(url, headers={"User-Agent": "valo-server"})
    with urllib.request.urlopen(req, timeout=30) as r:
        data = r.read(25 << 20)
        mime = r.headers.get_content_type()
    save_art(folder, name, data, mime)


def save_art(folder, name, data, mime):
    ext = {"image/png": ".png", "image/webp": ".webp"}.get(mime, ".jpg")
    for old in IMAGE:
        (folder / f"{name}{old}").unlink(missing_ok=True)
    (folder / f"{name}{ext}").write_bytes(data)


class Users:
    """users.json: {"name": {"salt": hex, "hash": hex, "admin": bool}};
    tokens.json: {"token": {"name", "device", "ip", "created", "last_used"}}."""

    def __init__(self, folder):
        self.folder = folder
        self.users = read_info(folder / "users.json")
        # older servers kept {"token": "name"}
        self.tokens = {
            t: v if isinstance(v, dict) else {"name": v}
            for t, v in read_info(folder / "tokens.json").items()
        }
        self.lock = threading.Lock()
        self.saved_at = 0

    @staticmethod
    def hash(password, salt):
        return hashlib.pbkdf2_hmac(
            "sha256", password.encode(), bytes.fromhex(salt), 200_000
        ).hex()

    @staticmethod
    def session_id(token):
        """What the app sees instead of the token."""
        return hashlib.sha256(token.encode()).hexdigest()[:16]

    def save(self, name, data):
        self.folder.mkdir(parents=True, exist_ok=True)
        path = self.folder / name
        path.write_text(json.dumps(data, indent=2))
        path.chmod(0o600)

    def add(self, name, password, admin=False, display_name=None):
        salt = secrets.token_hex(16)
        self.users[name] = {
            "salt": salt,
            "hash": self.hash(password, salt),
            "admin": admin,
        }
        if display_name:
            self.users[name]["display_name"] = display_name
        self.save("users.json", self.users)

    def find(self, name):
        """The stored username for `name`, ignoring case ("user" signs in as "User")."""
        if name in self.users:
            return name
        return next((n for n in self.users if n.lower() == name.lower()), None)

    def login(self, name, password, device=None, ip=None):
        name = self.find(name) or name
        u = self.users.get(name)
        if not u or not hmac.compare_digest(u["hash"], self.hash(password, u["salt"])):
            return None
        token = secrets.token_urlsafe(32)
        now = int(time.time())
        with self.lock:
            self.tokens[token] = {
                "name": name,
                "device": (str(device)[:100] if device else None),
                "ip": ip,
                "created": now,
                "last_used": now,
            }
            self.save("tokens.json", self.tokens)
        return token

    def owner(self, token, ip=None):
        """The user a token belongs to; notes that the session was used."""
        s = self.tokens.get(token)
        if not s:
            return None
        now = int(time.time())
        s["last_used"] = now
        if ip:
            s["ip"] = ip
        if now - self.saved_at > 60:  # don't write the file on every request
            with self.lock:
                self.saved_at = now
                self.save("tokens.json", self.tokens)
        return s["name"]

    def sessions(self, name, current):
        return [
            {
                "id": self.session_id(t),
                "device": s.get("device"),
                "ip": s.get("ip"),
                "created": s.get("created"),
                "last_used": s.get("last_used"),
                "current": t == current,
            }
            for t, s in list(self.tokens.items())
            if s["name"] == name
        ]

    def end_session(self, name, id):
        with self.lock:
            for t, s in list(self.tokens.items()):
                if s["name"] == name and self.session_id(t) == id:
                    del self.tokens[t]
                    self.save("tokens.json", self.tokens)
                    return True
        return False

    def is_admin(self, name):
        return bool(self.users.get(name, {}).get("admin"))

    def logout(self, token):
        with self.lock:
            if self.tokens.pop(token, None):
                self.save("tokens.json", self.tokens)

    def logout_all(self, name):
        with self.lock:
            self.tokens = {t: s for t, s in self.tokens.items() if s["name"] != name}
            self.save("tokens.json", self.tokens)

    def admins(self):
        return [n for n in self.users if self.is_admin(n)]

    def set_admin(self, name, admin):
        self.users[name]["admin"] = admin
        self.save("users.json", self.users)

    def set_password(self, name, password):
        salt = secrets.token_hex(16)
        self.users[name].update(salt=salt, hash=self.hash(password, salt))
        self.save("users.json", self.users)
        self.logout_all(name)

    def check_password(self, name, password):
        u = self.users.get(name)
        return bool(u) and hmac.compare_digest(u["hash"], self.hash(password, u["salt"]))

    def change_own_password(self, name, password, token):
        """The user's own change: other devices are signed out, this one stays,
        and an admin's "change your password" is done."""
        salt = secrets.token_hex(16)
        self.users[name].update(salt=salt, hash=self.hash(password, salt))
        self.users[name].pop("must_change_password", None)
        self.save("users.json", self.users)
        with self.lock:
            self.tokens = {t: s for t, s in self.tokens.items() if s["name"] != name or t == token}
            self.save("tokens.json", self.tokens)

    def must_change_password(self, name):
        return bool(self.users.get(name, {}).get("must_change_password"))

    def set_must_change_password(self, name, on):
        if on:
            self.users[name]["must_change_password"] = True
        else:
            self.users[name].pop("must_change_password", None)
        self.save("users.json", self.users)

    def delete(self, name):
        del self.users[name]
        self.save("users.json", self.users)
        self.logout_all(name)

    def listing(self, progress):
        sessions, used = {}, {}
        for s in list(self.tokens.values()):
            n = s["name"]
            sessions[n] = sessions.get(n, 0) + 1
            used[n] = max(used.get(n) or 0, s.get("last_used") or 0) or None
        return [
            {
                "name": n,
                "display_name": self.users[n].get("display_name"),
                "admin": self.is_admin(n),
                "must_change_password": self.must_change_password(n),
                "sessions": sessions.get(n, 0),
                "last_active": max(
                    (t for t in (progress.last_active(n), used.get(n)) if t), default=None
                ),
            }
            for n in sorted(self.users, key=str.lower)
        ]


class Progress:
    """progress.json: {"user": {"movie:<id>": {"position", "duration", "updated"}}}."""

    def __init__(self, folder):
        self.path = folder / "progress.json"
        self.data = read_info(self.path)
        self.lock = threading.Lock()

    @staticmethod
    def entry(key, e):
        kind, id = key.split(":", 1)
        pos, dur = e["position"], e["duration"]
        pct = round(100 * pos / dur) if dur > 0 else 0
        return {
            "type": kind,
            "id": id,
            "position": pos,
            "duration": dur,
            "percent": min(pct, 100),
            "completed": dur > 0 and pos >= 0.95 * dur,
            "updated": e["updated"],
        }

    def last_active(self, user):
        return max((e["updated"] for e in self.data.get(user, {}).values()), default=None)

    def forget(self, user):
        with self.lock:
            if self.data.pop(user, None) is not None:
                self.path.write_text(json.dumps(self.data, indent=2))

    def list(self, user):
        mine = self.data.get(user, {})
        out = [self.entry(k, e) for k, e in mine.items()]
        return sorted(out, key=lambda e: -e["updated"])

    def record(self, user, kind, id, position, duration):
        key = f"{kind}:{id}"
        with self.lock:
            e = {
                "position": position,
                "duration": duration,
                "updated": int(time.time()),
            }
            self.data.setdefault(user, {})[key] = e
            self.path.parent.mkdir(parents=True, exist_ok=True)
            self.path.write_text(json.dumps(self.data, indent=2))
            self.path.chmod(0o600)
        return self.entry(key, e)


def library_ids(lib, kind):
    data = lib.get()
    if kind == "movie":
        return {m["id"] for m in data["movies"]}
    return {
        e["id"]
        for s in data["series"]
        for season in s["seasons"]
        for e in season["episodes"]
    }


def make_handler(lib, users, progress, name, cols, reqs, addons, omdb_key=None, bot_token=None):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, fmt, *args):
            path = urlparse(self.path).path
            # streams, artwork and the log feed itself would drown everything else
            if not path.startswith(("/media/", "/art/", "/api/admin/logs")):
                log(f"{self.address_string()} {fmt % args}")

        def send_json(self, obj, status=200):
            body = json.dumps(obj).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def token(self):
            h = self.headers.get("Authorization", "")
            if h.startswith("Bearer "):
                return h[7:]
            return parse_qs(urlparse(self.path).query).get("token", [None])[0]

        def authorized(self):
            return not users.users or self.token() in users.tokens

        def user(self):
            """The signed-in user; None without sign-in, where progress has no owner."""
            return users.owner(self.token(), self.client_address[0])

        def is_bot(self):
            t = self.token()
            return bool(bot_token and t and hmac.compare_digest(t, bot_token))

        def query(self):
            return {k: v[0] for k, v in parse_qs(urlparse(self.path).query).items()}

        def json_body(self, body):
            try:
                req = json.loads(body) if body else {}
            except ValueError:
                return None
            return req if isinstance(req, dict) else None

        def requests_api(self, method, path, body):
            """/api/requests: the Discord bot (with --bot-token) or a signed-in user asks for titles."""
            bot = self.is_bot()
            user = None if bot else self.user()
            if not bot and not user:
                return self.send_json({"error": "sign in required"}, 401)
            if bot:
                reqs.bot_seen = time.time()
            parts = [unquote(p) for p in path.split("/")[3:] if p]
            if not parts and method == "POST":
                req = self.json_body(body)
                if req is None:
                    return self.send_json({"error": "bad request"}, 400)
                r = reqs.add(req, by=user)
                if not r:
                    return self.send_json({"error": "a title is required"}, 422)
                log(f"request: {r['title']} by {r['requested_by'] or 'someone'} via {r['source']}")
                return self.send_json(r, 201)
            if not parts and method == "GET":
                since = int(self.query().get("updated_after") or 0)
                mine = [r for r in reqs.items if r["updated"] > since]
                if not bot:
                    mine = [r for r in mine if r["requested_by"] == user]
                return self.send_json(mine)
            if len(parts) == 1 and method == "PATCH" and bot:
                # the bot reports back: did the requester get the message, which message is it
                req = self.json_body(body) or {}
                with reqs.lock:
                    r = reqs.find(parts[0])
                    if not r:
                        return self.send_json({"error": "not found"}, 404)
                    for k in ("notified", "message_id"):
                        if k in req:
                            r[k] = req[k]
                    reqs.save()
                return self.send_json(r)
            if len(parts) == 1 and method == "GET":
                r = reqs.find(parts[0])
                if not r or (not bot and r["requested_by"] != user):
                    return self.send_json({"error": "not found"}, 404)
                return self.send_json(r)
            return self.send_json({"error": "not found"}, 404)

        def item_api(self, method, kind, id, rest, req, body):
            """/api/admin/movies/<id>[/fetch|/art/<which>], same for series."""
            data = lib.get()
            items = data["movies"] if kind == "movies" else data["series"]
            item = next((i for i in items if i["id"] == id), None)
            files = lib.item_files(kind, id) if item else None
            if not files:
                return self.send_json({"error": "no such title"}, 404)
            info, folder, stem = files
            if rest == ["fetch"] and method == "POST":
                if not omdb_key:
                    return self.send_json({"error": "IMDb lookups need --omdb-key"}, 501)
                try:
                    found = omdb(
                        omdb_key,
                        req.get("title") or item["title"],
                        req.get("year") or item.get("year"),
                        "series" if kind == "series" else "movie",
                        req.get("imdb_id"),
                    )
                except LookupError as e:
                    return self.send_json({"error": str(e)}, 422)
                except OSError as e:
                    return self.send_json({"error": f"IMDb lookup failed: {e}"}, 502)
                log(f"fetched {found.get('title')} from IMDb")
                return self.send_json(found)
            names = {
                "poster": [f"{stem}-poster" if stem else "poster"],
                "backdrop": [f"{stem}-fanart" if stem else "fanart"],
                "thumb": [f"{stem}-thumb" if stem else "thumb"],
                "logo": [None if stem else "logo"],
            }
            # every name the scan accepts for each, so a removal leaves none behind
            aliases = {
                "poster": [stem, f"{stem}-poster"] if stem else ["poster", "folder", "cover"],
                "backdrop": [f"{stem}-fanart"] if stem else ["fanart", "backdrop", "background"],
                "thumb": [f"{stem}-thumb"] if stem else ["thumb", "landscape"],
                "logo": [] if stem else ["logo", "clearlogo"],
            }
            if len(rest) == 2 and rest[0] == "art" and method == "PUT":
                which = names.get(rest[1], [None])[0]
                if not which:
                    return self.send_json({"error": "a loose movie file can't have this artwork; give it its own folder"}, 422)
                mime = self.headers.get("Content-Type", "image/jpeg").split(";")[0]
                if not body or not mime.startswith("image/"):
                    return self.send_json({"error": "send the image as the body"}, 422)
                save_art(folder, which, body, mime)
                lib.get(fresh=True)
                log(f"new {rest[1]} for {item['title']}")
                return self.send_json({})
            if not rest and method == "GET":
                # the library entry plus everything info.json keeps (imdb_id, …)
                return self.send_json({**read_info(info), **item})
            if rest or method != "PATCH":
                return self.send_json({"error": "not found"}, 404)
            allowed = set(EXTRAS) | {"title", "year", "overview", "genres", "rating", "duration", "imdb_id", "cast"}
            fields = {k: v for k, v in req.items() if k in allowed}
            for art in ("poster", "backdrop", "thumb", "logo"):
                url = req.get(art)
                if art in req and url is None:
                    for n in aliases[art]:
                        for ext in IMAGE:
                            (folder / f"{n}{ext}").unlink(missing_ok=True)
                    continue
                if isinstance(url, str) and url.startswith(("http://", "https://")):
                    which = names[art][0]
                    if not which:
                        continue
                    try:
                        download(url, folder, which)
                    except OSError as e:
                        return self.send_json({"error": f"couldn't download the {art}: {e}"}, 502)
            if fields:
                write_info(info, fields)
            lib.get(fresh=True)
            log(f"edited {item['title']}: {', '.join(sorted(set(fields) | {k for k in ('poster', 'backdrop', 'thumb', 'logo') if k in req}))}")
            return self.send_json({})

        def body(self):
            length = int(self.headers.get("Content-Length") or 0)
            return self.rfile.read(length) if length else b""

        def admin(self, method, path, body):
            """/api/admin/...: users and library, for admins only."""
            user = self.user()
            if not user:
                return self.send_json({"error": "sign in required"}, 401)
            if not users.is_admin(user):
                return self.send_json({"error": "admins only"}, 403)
            if method == "PUT":
                req = {}  # an image upload
            else:
                req = self.json_body(body)
                if req is None:
                    return self.send_json({"error": "bad request"}, 400)
            parts = [unquote(p) for p in path.split("/")[3:]]
            if parts == ["status"] and method == "GET":
                services = [
                    {
                        "name": "IMDb lookups (OMDb)",
                        "ok": bool(omdb_key),
                        "detail": "API key set" if omdb_key else "no --omdb-key",
                    }
                ]
                if bot_token:
                    seen = reqs.bot_seen
                    ok = bool(seen and time.time() - seen < 600)
                    detail = f"last seen {int(time.time() - seen)} s ago" if seen else "hasn't connected yet"
                    services.append({"name": "Discord bot", "ok": ok, "detail": detail})
                else:
                    services.append({"name": "Discord bot", "ok": False, "detail": "no --bot-token"})
                data = lib.get()
                files = [
                    f["size"]
                    for f in [m["file"] for m in data["movies"]]
                    + [e["file"] for s in data["series"] for se in s["seasons"] for e in se["episodes"]]
                ]
                no_poster = [
                    {"type": t, "id": i["id"], "title": i["title"], "year": i.get("year")}
                    for t, key in (("movie", "movies"), ("series", "series"))
                    for i in data[key]
                    if not i.get("poster")
                ]
                return self.send_json(
                    {
                        "version": VERSION,
                        "uptime": int(time.time() - STARTED),
                        "library": {
                            "movies": len(data["movies"]),
                            "series": len(data["series"]),
                            "episodes": len(files) - len(data["movies"]),
                            "size": sum(files),
                            "users": len(users.users),
                        },
                        "services": services,
                        "health": [
                            {"label": "Missing poster", "count": len(no_poster), "titles": no_poster[:100]},
                            {
                                "label": "Episode files without SxxEyy in the name",
                                "count": len(lib.skipped),
                                "items": lib.skipped[:100],
                            },
                        ],
                    }
                )
            if parts[:1] == ["sync"] and len(parts) == 2 and method == "POST":
                if parts[1] not in ("folders", "movies", "series"):
                    return self.send_json({"error": "not found"}, 404)
                log(f"sync {parts[1]} started by {user}")
                r = lib.sync(parts[1], apply=self.query().get("apply") == "1")
                log(f"sync {parts[1]}: {len(r['added'])} new, {len(r['removed'])} removed"
                    + ("" if r["applied"] else " (dry run)"))
                return self.send_json(r)
            if parts == ["logs"] and method == "GET":
                return self.send_json(log.after(int(self.query().get("after") or 0)))
            if parts == ["analytics"] and method == "GET":
                days = max(1, min(int(self.query().get("days") or 30), 366))
                return self.send_json(analytics(progress, lib, days))
            if parts[:2] == ["analytics", "users"] and len(parts) == 3 and method == "GET":
                if parts[2] not in users.users:
                    return self.send_json({"error": f"no user named {parts[2]}"}, 404)
                days = max(1, min(int(self.query().get("days") or 30), 366))
                return self.send_json(user_analytics(progress, users, lib, parts[2], days))
            if parts == ["requests"] and method == "GET":
                return self.send_json(sorted(reqs.items, key=lambda r: -r["created"]))
            if parts[:1] == ["requests"] and len(parts) == 2:
                if method == "PATCH":
                    status = req.get("status")
                    if status not in Requests.STATUSES:
                        return self.send_json({"error": "unknown status"}, 422)
                    r = reqs.update(parts[1], status, req.get("note"))
                    if not r:
                        return self.send_json({"error": "no such request"}, 404)
                    log(f"request {r['title']}: {status} by {user}")
                    return self.send_json(r)
                if method == "DELETE":
                    if not reqs.delete(parts[1]):
                        return self.send_json({"error": "no such request"}, 404)
                    return self.send_json({})
            if parts in (["movies"], ["series"]) and method == "GET":
                # this server only knows titles with files, so it's the library
                return self.send_json(lib.get()[parts[0]])
            if parts[:1] in (["movies"], ["series"]) and len(parts) >= 2:
                return self.item_api(method, parts[0], parts[1], parts[2:], req, body)
            if parts == ["collections"] and method == "GET":
                data = lib.get()
                known = {("movie", m["id"]): m for m in data["movies"]}
                known.update({("series", x["id"]): x for x in data["series"]})
                out = []
                for c in cols.items:
                    items = []
                    for it in c["items"]:
                        m = known.get((it["type"], it["id"]), {})
                        items.append({**it, "title": m.get("title"), "year": m.get("year"), "poster": m.get("poster")})
                    out.append({**c, "items": items})
                return self.send_json(out)
            if parts == ["collections"] and method == "POST":
                if not str(req.get("name", "")).strip():
                    return self.send_json({"error": "a collection needs a name"}, 422)
                c = cols.save_one(None, req)
                lib.get(fresh=True)
                log(f"collection {c['name']} created by {user}")
                return self.send_json(c, 201)
            if parts[:1] == ["collections"] and len(parts) == 2:
                if method == "PATCH":
                    c = cols.save_one(parts[1], req)
                    if not c:
                        return self.send_json({"error": "no such collection"}, 404)
                    lib.get(fresh=True)
                    log(f"collection {c['name']} changed by {user}")
                    return self.send_json(c)
                if method == "DELETE":
                    if not cols.delete(parts[1]):
                        return self.send_json({"error": "no such collection"}, 404)
                    lib.get(fresh=True)
                    return self.send_json({})
            if parts == ["addons"] and method == "POST":
                try:
                    a = addons.add(str(req.get("url", "")), user)
                except AddonError as e:
                    return self.send_json({"error": str(e)}, 422)
                log(f"addon {a['name']} added by {user}")
                return self.send_json(a, 201)
            if parts[:1] == ["addons"] and len(parts) == 2 and method == "DELETE":
                if not addons.delete(parts[1]):
                    return self.send_json({"error": "no such addon"}, 404)
                return self.send_json({})
            if parts == ["rescan"] and method == "POST":
                data = lib.get(fresh=True)
                episodes = sum(
                    len(se["episodes"]) for s in data["series"] for se in s["seasons"]
                )
                return self.send_json(
                    {
                        "movies": len(data["movies"]),
                        "series": len(data["series"]),
                        "episodes": episodes,
                    }
                )
            if parts == ["users"] and method == "GET":
                return self.send_json(users.listing(progress))
            if parts == ["users"] and method == "POST":
                name = str(req.get("username", "")).strip()
                password = str(req.get("password", ""))
                if not name or len(name) > 64 or not password:
                    return self.send_json(
                        {"error": "a username and a password are required"}, 422
                    )
                if users.find(name):
                    return self.send_json({"error": f"{users.find(name)} already exists"}, 409)
                display = str(req.get("display_name") or "").strip()[:64] or None
                users.add(name, password, bool(req.get("admin")), display)
                return self.send_json({"name": name}, 201)
            if len(parts) < 2 or parts[0] != "users":
                return self.send_json({"error": "not found"}, 404)
            name = parts[1]
            if name not in users.users:
                return self.send_json({"error": f"no user named {name}"}, 404)
            last_admin = users.admins() == [name]
            if parts[2:] == ["logout"] and method == "POST":
                users.logout_all(name)
                return self.send_json({})
            if parts[2:]:
                return self.send_json({"error": "not found"}, 404)
            if method == "PATCH":
                if "admin" in req:
                    if not req["admin"] and last_admin:
                        return self.send_json(
                            {"error": "the server needs at least one admin"}, 422
                        )
                    users.set_admin(name, bool(req["admin"]))
                if req.get("password"):
                    users.set_password(name, str(req["password"]))
                if "must_change_password" in req:
                    users.set_must_change_password(name, bool(req["must_change_password"]))
                if "display_name" in req:
                    users.users[name]["display_name"] = str(req["display_name"] or "").strip()[:64] or None
                    users.save("users.json", users.users)
                return self.send_json({})
            if method == "DELETE":
                if last_admin:
                    return self.send_json(
                        {"error": "can't delete the only admin"}, 422
                    )
                users.delete(name)
                progress.forget(name)
                return self.send_json({})
            return self.send_json({"error": "method not allowed"}, 405)

        def do_PUT(self):
            path = urlparse(self.path).path
            if path.startswith("/api/admin/"):
                return self.admin("PUT", path, self.body())
            self.send_json({"error": "not found"}, 404)

        def do_PATCH(self):
            path = urlparse(self.path).path
            if path.startswith("/api/admin/"):
                return self.admin("PATCH", path, self.body())
            if path.startswith("/api/requests/"):
                return self.requests_api("PATCH", path, self.body())
            self.send_json({"error": "not found"}, 404)

        def do_DELETE(self):
            path = urlparse(self.path).path
            if path.startswith("/api/admin/"):
                return self.admin("DELETE", path, self.body())
            if path.startswith("/api/sessions/"):
                self.body()
                user = self.user()
                if not user:
                    return self.send_json({"error": "sign in required"}, 401)
                if not users.end_session(user, unquote(path[len("/api/sessions/"):])):
                    return self.send_json({"error": "no such session"}, 404)
                log(f"{user} signed out a device")
                return self.send_json({})
            self.send_json({"error": "not found"}, 404)

        def do_POST(self):
            path = urlparse(self.path).path
            body = self.body()
            if path.startswith("/api/admin/"):
                return self.admin("POST", path, body)
            if path == "/api/requests" or path.startswith("/api/requests/"):
                return self.requests_api("POST", path, body)
            if path == "/api/login":
                try:
                    req = json.loads(body)
                    token = users.login(
                        str(req["username"]),
                        str(req["password"]),
                        req.get("device"),
                        self.client_address[0],
                    )
                except (ValueError, KeyError, TypeError):
                    return self.send_json({"error": "bad request"}, 400)
                if not token:
                    time.sleep(1)  # slow down guessing
                    return self.send_json({"error": "wrong username or password"}, 401)
                user = users.find(str(req["username"]))
                return self.send_json(
                    {
                        "token": token,
                        "name": user,
                        "admin": users.is_admin(user),
                        "must_change_password": users.must_change_password(user),
                    }
                )
            if path == "/api/logout":
                users.logout(self.token())
                return self.send_json({})
            if path == "/api/password":
                user = self.user()
                if not user:
                    return self.send_json({"error": "sign in required"}, 401)
                try:
                    req = json.loads(body)
                    current, new = str(req["current_password"]), str(req["password"])
                except (ValueError, KeyError, TypeError):
                    return self.send_json({"error": "bad request"}, 422)
                if not new:
                    return self.send_json({"error": "the new password can't be empty"}, 422)
                if not users.check_password(user, current):
                    time.sleep(1)  # slow down guessing
                    return self.send_json({"error": "wrong current password"}, 403)
                users.change_own_password(user, new, self.token())
                log(f"{user} changed their password")
                return self.send_json({})
            if path == "/api/progress":
                user = self.user()
                if not user:
                    return self.send_json({"error": "sign in required"}, 401)
                try:
                    req = json.loads(body)
                    kind, id = req["type"], str(req["id"])
                    position, duration = float(req["position"]), float(
                        req.get("duration") or 0
                    )
                    if kind not in ("movie", "episode") or position < 0 or duration < 0:
                        raise ValueError
                except (ValueError, KeyError, TypeError):
                    return self.send_json({"error": "bad request"}, 422)
                if id not in library_ids(lib, kind):
                    return self.send_json({"error": "not found"}, 404)
                return self.send_json(
                    progress.record(user, kind, id, position, duration)
                )
            self.send_json({"error": "not found"}, 404)

        def do_HEAD(self):
            self.do_GET(head=True)

        def do_GET(self, head=False):
            path = urlparse(self.path).path
            if path == "/api/info":
                return self.send_json(
                    {"name": name, "auth": bool(users.users), "version": 1}
                )
            if path == "/api/requests" or path.startswith("/api/requests/"):
                return self.requests_api("GET", path, b"")
            if not self.authorized():
                return self.send_json({"error": "sign in required"}, 401)
            if path == "/api/library":
                return self.send_json(lib.get())
            if path == "/api/me":
                user = self.user()
                if not user:
                    return self.send_json({"error": "sign in required"}, 401)
                return self.send_json(
                    {
                        "name": user,
                        "admin": users.is_admin(user),
                        "must_change_password": users.must_change_password(user),
                    }
                )
            if path.startswith("/api/admin/"):
                return self.admin("GET", path, b"")
            if path == "/api/addons":
                return self.send_json(addons.items)
            if path == "/api/sessions":
                user = self.user()
                if not user:
                    return self.send_json({"error": "sign in required"}, 401)
                return self.send_json(users.sessions(user, self.token()))
            if path == "/api/progress":
                user = self.user()
                if not user:
                    return self.send_json({"error": "sign in required"}, 401)
                return self.send_json(progress.list(user))
            for prefix in ("/media/", "/art/"):
                if path.startswith(prefix):
                    f = (lib.root / unquote(path[len(prefix) :])).resolve()
                    if lib.root in f.parents and f.is_file():
                        return self.send_file(f, head)
            self.send_json({"error": "not found"}, 404)

        def send_file(self, f, head):
            size = f.stat().st_size
            start, end = 0, size - 1
            m = re.match(r"bytes=(\d*)-(\d*)", self.headers.get("Range", ""))
            if m and (m.group(1) or m.group(2)):
                if m.group(1):
                    start = int(m.group(1))
                    end = int(m.group(2)) if m.group(2) else end
                else:
                    start = max(0, size - int(m.group(2)))
                if start >= size:
                    self.send_response(416)
                    self.send_header("Content-Range", f"bytes */{size}")
                    self.send_header("Content-Length", "0")
                    return self.end_headers()
                end = min(end, size - 1)
                self.send_response(206)
                self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
            else:
                self.send_response(200)
            self.send_header(
                "Content-Type",
                mimetypes.guess_type(f.name)[0] or "application/octet-stream",
            )
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Content-Length", str(end - start + 1))
            self.end_headers()
            if head:
                return
            with open(f, "rb") as fh:
                fh.seek(start)
                left = end - start + 1
                try:
                    while left > 0:
                        chunk = fh.read(min(1 << 20, left))
                        if not chunk:
                            break
                        self.wfile.write(chunk)
                        left -= len(chunk)
                except (BrokenPipeError, ConnectionResetError):
                    pass  # player seeked or closed

    return Handler


def main():
    ap = argparse.ArgumentParser(description="Valo reference media server")
    ap.add_argument("media", type=Path, help="folder containing Movies/ and Series/")
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--host", default="0.0.0.0")
    ap.add_argument("--name", default=None, help="name shown in the app")
    ap.add_argument(
        "--add-user", metavar="NAME", help="create/update a user, then exit"
    )
    ap.add_argument(
        "--admin", action="store_true", help="with --add-user: the user may manage the server from the app"
    )
    ap.add_argument(
        "--omdb-key",
        default=os.environ.get("OMDB_API_KEY"),
        help="OMDb API key, for fetching details from IMDb on the Admin page (or OMDB_API_KEY)",
    )
    ap.add_argument(
        "--bot-token",
        default=os.environ.get("VALO_BOT_TOKEN"),
        help="secret the Discord bot sends to post requests (or VALO_BOT_TOKEN)",
    )
    a = ap.parse_args()
    root = a.media.expanduser().resolve()
    data = root / ".valo"
    users = Users(data)
    if a.add_user:
        pw = getpass.getpass(f"Password for {a.add_user}: ")
        if pw != getpass.getpass("Again: "):
            raise SystemExit("passwords don't match")
        users.add(a.add_user, pw, a.admin)
        role = "admin" if a.admin else "user"
        print(f"User {a.add_user} saved ({role}). Sign-in is now required.")
        return
    name = a.name or f"{os.uname().nodename} media"
    progress = Progress(data)
    cols = Collections(data)
    reqs = Requests(data)
    addons = Addons(data)
    handler = make_handler(
        Library(root, cols), users, progress, name, cols, reqs, addons, a.omdb_key, a.bot_token
    )
    srv = ThreadingHTTPServer((a.host, a.port), handler)
    log(
        f"Serving {root} on http://{a.host}:{a.port}  (sign-in {'on' if users.users else 'off'})"
    )
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
