package io.github.okaza03.valo

import android.Manifest
import android.app.Application
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.AndroidViewModel
import coil3.ImageLoader
import coil3.compose.setSingletonImageLoaderFactory
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.crossfade
import io.github.okaza03.valo.cast.Caster
import io.github.okaza03.valo.ui.Arrivals
import io.github.okaza03.valo.ui.BG
import io.github.okaza03.valo.ui.BottomBar
import io.github.okaza03.valo.ui.DetailsScreen
import io.github.okaza03.valo.ui.GridScreen
import io.github.okaza03.valo.ui.HomeScreen
import io.github.okaza03.valo.ui.LoadingScreen
import io.github.okaza03.valo.ui.LocalCast
import io.github.okaza03.valo.ui.LoginScreen
import io.github.okaza03.valo.ui.MiniCast
import io.github.okaza03.valo.ui.PasswordPrompt
import io.github.okaza03.valo.ui.PersonScreen
import io.github.okaza03.valo.ui.PlayerScreen
import io.github.okaza03.valo.ui.ProfileScreen
import io.github.okaza03.valo.ui.RemoteScreen
import io.github.okaza03.valo.ui.SetupScreen
import io.github.okaza03.valo.ui.Splash
import io.github.okaza03.valo.ui.StreamingScreen
import io.github.okaza03.valo.ui.ValoTheme
import io.github.okaza03.valo.ui.WhatsNew

/** Keeps casting going while the activity comes and goes. */
class CastHolder(app: Application) : AndroidViewModel(app) {
    private var caster: Caster? = null

    fun get(model: AppModel) = caster ?: Caster(getApplication(), model).also { caster = it }

    override fun onCleared() {
        caster?.close()
    }
}

class MainActivity : FragmentActivity() {
    private val model: AppModel by viewModels()
    private val holder: CastHolder by viewModels()

    // only for the "Playing on …" notification with its buttons; casting works without it
    private val askNotify = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        // light icons always: the app is dark, whatever the phone's own theme
        val bars = SystemBarStyle.dark(Color.TRANSPARENT)
        enableEdgeToEdge(bars, bars)
        super.onCreate(savedInstanceState)
        val cast = holder.get(model)
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
                CompositionLocalProvider(LocalCast provides cast) {
                    // the phone carries on streaming when Back leaves the app
                    BackHandler { if (!model.back()) { if (cast.now != null) moveTaskToBack(true) else finish() } }
                    LaunchedEffect(cast.device) {
                        val missing = Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(
                            this@MainActivity, Manifest.permission.POST_NOTIFICATIONS,
                        ) != PackageManager.PERMISSION_GRANTED
                        if (cast.device != null && missing) askNotify.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                    Root(model, cast)
                }
            }
        }
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

private val Screen.tab get() = this == Screen.Home || this is Screen.Grid || this == Screen.Streaming || this == Screen.Profile

@Composable
private fun Root(model: AppModel, cast: Caster) {
    val s = model.screen
    val player = s is Screen.Player
    val lib = model.library
    Box(Modifier.fillMaxSize().background(BG)) {
        // the video runs under the system bars; everything else stays clear of them
        Column(Modifier.fillMaxSize().then(if (player) Modifier else Modifier.statusBarsPadding())) {
            Box(Modifier.weight(1f).fillMaxWidth()) { App(model) }
            if (cast.now != null && !player) MiniCast(model, cast)
            when {
                s.tab && lib != null -> BottomBar(model)
                !player -> Box(Modifier.navigationBarsPadding())
            }
        }
        // one popup at a time: What's new, then the password an admin asked for, then new titles
        val notes = model.whatsNew
        val home = !model.splash && s == Screen.Home
        if (notes != null && home) WhatsNew(notes, model::closeWhatsNew)
        if (notes == null && home && model.askPassword) PasswordPrompt(model)
        // new titles, once the user is browsing (never over the player)
        val browsing = s == Screen.Home || s is Screen.Grid
        if (lib != null && notes == null && !model.askPassword && !model.splash && browsing &&
            model.query.isBlank() && (model.arrivals.isNotEmpty() || model.departures.isNotEmpty())
        ) {
            Arrivals(model, lib)
        }
        if (model.splash) Splash(ready = s !is Screen.Loading, hold = s == Screen.Home) { model.splash = false }
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
