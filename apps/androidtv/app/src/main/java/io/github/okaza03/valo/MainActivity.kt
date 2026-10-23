package io.github.okaza03.valo

import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import coil3.ImageLoader
import coil3.compose.setSingletonImageLoaderFactory
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.crossfade
import io.github.okaza03.valo.ui.Arrivals
import io.github.okaza03.valo.ui.DetailsScreen
import io.github.okaza03.valo.ui.GridScreen
import io.github.okaza03.valo.ui.HomeScreen
import io.github.okaza03.valo.ui.LoadingScreen
import io.github.okaza03.valo.ui.ValoTheme
import io.github.okaza03.valo.ui.LoginScreen
import io.github.okaza03.valo.ui.PlayerKeys
import io.github.okaza03.valo.ui.PasswordPrompt
import io.github.okaza03.valo.ui.PersonScreen
import io.github.okaza03.valo.ui.RemoteScreen
import io.github.okaza03.valo.ui.StreamingScreen
import io.github.okaza03.valo.ui.PlayerScreen
import io.github.okaza03.valo.ui.ProfileScreen
import io.github.okaza03.valo.ui.SetupScreen
import io.github.okaza03.valo.ui.Splash
import io.github.okaza03.valo.ui.WhatsNew

/** Holding OK this long opens the player's subtitles. */
private const val HOLD_MS = 500L

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // images on our server need the sign-in token; ones on a CDN don't get it
        val images = Client.http.newBuilder().addInterceptor { chain ->
            val req = chain.request()
            val auth = model.client?.authFor(req.url.toString())
            chain.proceed(if (auth != null) req.newBuilder().header("Authorization", auth).build() else req)
        }.build()
        setContent {
            setSingletonImageLoaderFactory { ctx ->
                ImageLoader.Builder(ctx)
                    .components { add(OkHttpNetworkFetcherFactory(callFactory = { images })) }
                    .crossfade(true)
                    .build()
            }
            ValoTheme {
                BackHandler { if (!model.back()) finish() }
                Box {
                    App(model)
                    // one popup at a time: What's new, then the password an admin asked for, then new titles
                    val notes = model.whatsNew
                    if (notes != null && !model.splash && model.screen == Screen.Home) {
                        WhatsNew(notes, model::closeWhatsNew)
                    }
                    val home = !model.splash && model.screen == Screen.Home
                    if (notes == null && home && model.askPassword) PasswordPrompt(model)
                    // new titles, once the user is browsing (never over the player)
                    val lib = model.library
                    val browsing = model.screen == Screen.Home || model.screen is Screen.Grid
                    if (lib != null && notes == null && !model.askPassword && !model.splash && browsing &&
                        model.query.isBlank() && model.arrivals.isNotEmpty()
                    ) {
                        Arrivals(model, lib)
                    }
                    if (model.splash) Splash(
                        ready = model.screen !is Screen.Loading,
                        hold = model.screen == Screen.Home,
                    ) { model.splash = false }
                }
            }
        }
    }

    /** OK went down while the player was up; whether holding it already opened the subtitles. */
    private var okDown = false
    private var held = false

    // the remote's keys go to the player even when focus slipped off it (see PlayerKeys)
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val hold = PlayerKeys.onHold
        if (hold != null && event.keyCode == KeyEvent.KEYCODE_CAPTIONS) {
            if (event.action == KeyEvent.ACTION_UP) hold()
            return true
        }
        // holding OK opens the subtitles; a short press still reaches the player, on release
        val ok = event.keyCode == KeyEvent.KEYCODE_DPAD_CENTER || event.keyCode == KeyEvent.KEYCODE_ENTER
        if (hold != null && ok) {
            when {
                event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0 -> {
                    okDown = true
                    held = false
                }
                event.action == KeyEvent.ACTION_DOWN -> if (okDown && !held && (event.isLongPress || event.eventTime - event.downTime >= HOLD_MS)) {
                    held = true
                    hold()
                }
                event.action == KeyEvent.ACTION_UP && okDown -> {
                    okDown = false
                    if (!held) {
                        toPlayer(KeyEvent(event.downTime, event.downTime, KeyEvent.ACTION_DOWN, event.keyCode, 0))
                        toPlayer(event)
                    }
                }
                else -> return toPlayer(event)
            }
            return true
        }
        return toPlayer(event)
    }

    private fun toPlayer(event: KeyEvent): Boolean {
        val v = PlayerKeys.view
        if (v != null && v.isAttachedToWindow && !v.hasFocus() && event.keyCode != KeyEvent.KEYCODE_BACK) {
            v.requestFocus()
            if (v.dispatchKeyEvent(event)) return true
        }
        return super.dispatchKeyEvent(event)
    }

    override fun onResume() {
        super.onResume()
        model.resumed()
    }

    override fun onStop() {
        super.onStop()
        model.pushNow()
    }
}

@Composable
private fun App(model: AppModel) {
    val lib = model.library
    when (val s = model.screen) {
        is Screen.Loading -> LoadingScreen(s.message)
        is Screen.Setup -> SetupScreen(model, s.error)
        is Screen.Login -> LoginScreen(model, s.error)
        Screen.Home -> if (lib != null) HomeScreen(model, lib)
        is Screen.Grid -> if (lib != null) GridScreen(model, lib, s.kind)
        is Screen.Details -> if (lib != null) {
            // a title from the addons that isn't on the server has its own page
            lib.find(s.itemId)?.let { DetailsScreen(model, lib, it) }
                ?: model.discover.remote[s.itemId]?.let { RemoteScreen(model, lib, it) }
        }
        is Screen.Player -> lib?.find(s.itemId)?.let { PlayerScreen(model, it, s) }
        is Screen.Person -> if (lib != null) PersonScreen(model, lib, s.name)
        Screen.Profile -> if (lib != null) ProfileScreen(model, lib)
        Screen.Streaming -> if (lib != null) StreamingScreen(model, lib)
    }
}
