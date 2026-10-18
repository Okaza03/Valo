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
import io.github.okaza03.valo.ui.PersonScreen
import io.github.okaza03.valo.ui.PlayerScreen
import io.github.okaza03.valo.ui.ProfileScreen
import io.github.okaza03.valo.ui.SetupScreen
import io.github.okaza03.valo.ui.Splash
import io.github.okaza03.valo.ui.WhatsNew

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
                    val notes = model.whatsNew
                    if (notes != null && !model.splash && model.screen == Screen.Home) {
                        WhatsNew(notes, model::closeWhatsNew)
                    }
                    // new titles, once the user is browsing (never over the player)
                    val lib = model.library
                    val browsing = model.screen == Screen.Home || model.screen is Screen.Grid
                    if (lib != null && notes == null && !model.splash && browsing && model.query.isBlank() && model.arrivals.isNotEmpty()) {
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

    // the remote's keys go to the player even when focus slipped off it (see PlayerKeys)
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
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
        is Screen.Details -> lib?.find(s.itemId)?.let { DetailsScreen(model, lib, it) }
        is Screen.Player -> lib?.find(s.itemId)?.let { PlayerScreen(model, it, s) }
        is Screen.Person -> if (lib != null) PersonScreen(model, lib, s.name)
        Screen.Profile -> if (lib != null) ProfileScreen(model, lib)
    }
}
