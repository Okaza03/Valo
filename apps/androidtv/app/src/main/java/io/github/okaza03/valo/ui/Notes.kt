// The text of "What's new" and "New on <server>", for the TV and phone apps.
package io.github.okaza03.valo.ui

import io.github.okaza03.valo.Season
import io.github.okaza03.valo.Updates

class Note(val title: String, val text: String)

/**
 * Newest last. Changes on TVs and phones only; leave small fixes out, and admin
 * features too: they're for the server's admin, who knows about them.
 */
private val NOTES: List<Pair<String, List<Note>>> = listOf(
    "0.8.2" to listOf(
        Note("Updates in one go", "Choosing Update downloads the new version and opens the installer right away."),
        Note("What's new", "After an update, this page shows what changed."),
    ),
    "0.8.3" to listOf(
        Note("Subtitles", "Subtitles in a movie's or series' Subs folder on the server can be picked with the subtitle button, and the language you pick comes on by itself next time."),
    ),
    "0.8.5" to listOf(
        Note("Controls that come back", "After picking subtitles, any button on the remote brings the player's controls back. They show the title, and a Next button for series."),
        Note("My list", "Hold OK on a movie or series, or use + My list on its page, to keep it in the My list row on the home page."),
        Note("Cast and more", "Details pages show the cast and crew (open anyone to see their other titles), You might also like, file details and critics' quotes."),
        Note("Your profile", "Profile in the top-right menu has your watching stats, recently watched, and every device you're signed in on."),
        Note("New arrivals", "Movies and series added to your server since you last looked are shown when you open Valo."),
    ),
    "0.8.10" to listOf(
        Note("Popups you can read", "What's new and New arrivals keep the remote to themselves, and long notes scroll with up and down."),
        Note("Streaming and requests", "If your server's admin has added Stremio addons, Home has rows like Popular and Netflix, and the new Streaming page shows what's on each service. Titles that aren't on the server can be requested."),
        Note("Search beyond the server", "Search also finds movies and series the server doesn't have, so you can open them and ask for them."),
        Note("Everything they've been in", "A person's page lists all their movies and series, and which of them are on the server."),
        Note("Subtitles, your way", "Hold OK while watching to pick subtitles, including ones from the addons, and to move them earlier or later when they're out of sync."),
        Note("Your home page, your way", "Your server's admin can choose which rows show on Home, and in what order."),
        Note("Change your password", "When your server's admin asks you for a new password, Valo asks each time it opens until you change it."),
    ),
    "0.8.11" to listOf(
        Note("Removed titles", "New arrivals also lists movies and series taken off the server, and the library reloads when you come back to Valo, so they don't linger."),
    ),
)

/** The notes for versions after `seen`, up to this build; a fallback when none were written. */
fun notesSince(seen: String): List<Note> {
    val notes = NOTES.filter { (v, _) -> Updates.newer(v, seen) && !Updates.newer(v, Updates.VERSION) }
        .flatMap { it.second }
    return notes.ifEmpty { listOf(Note("Fixes and small improvements", "Valo ${Updates.VERSION} makes things work better.")) }
}

/** "1, 2 and 3" */
fun andList(parts: List<String>) =
    if (parts.size < 2) parts.joinToString("") else parts.dropLast(1).joinToString(", ") + " and " + parts.last()

/** "2 new movies, 1 new series and 3 new seasons" */
fun summary(movies: Int, series: Int, seasons: Int, removed: Int) = andList(
    listOfNotNull(
        when (movies) { 0 -> null; 1 -> "1 new movie"; else -> "$movies new movies" },
        when (series) { 0 -> null; 1 -> "1 new series"; else -> "$series new series" },
        when (seasons) { 0 -> null; 1 -> "1 new season"; else -> "$seasons new seasons" },
        when (removed) { 0 -> null; 1 -> "1 title removed"; else -> "$removed titles removed" },
    ),
)

/** "Season 2", "Seasons 1–3", "Seasons 1, 3 and 5", "Season 1 and Specials", "Season 1, Specials and Featurettes" */
fun seasonsLabel(nums: List<Int>): String {
    val n = nums.filter { it > 0 && it != Season.FEATURETTES }.distinct().sorted()
    val numbered = when {
        n.isEmpty() -> null
        n.size == 1 -> "Season ${n[0]}"
        n.last() - n.first() == n.size - 1 -> "Seasons ${n.first()}–${n.last()}"
        else -> "Seasons " + andList(n.map { it.toString() })
    }
    return andList(
        listOfNotNull(
            numbered,
            "Specials".takeIf { 0 in nums },
            "Featurettes".takeIf { Season.FEATURETTES in nums },
        ),
    )
}

