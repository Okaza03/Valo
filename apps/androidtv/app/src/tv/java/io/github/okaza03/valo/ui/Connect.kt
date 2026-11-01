// The screens before the library: loading, picking a server, signing in.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel

@Composable
fun LoadingScreen(message: String) {
    Box(Modifier.fillMaxSize().background(BG), contentAlignment = Alignment.Center) {
        Text(message, color = MUTED, fontSize = 18.sp)
    }
}

/** A centered column of fixed width, like a dialog. */
@Composable
private fun Panel(title: String, sub: String, content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().background(BG), contentAlignment = Alignment.Center) {
        Column(Modifier.width(520.dp)) {
            Text(title, fontSize = 32.sp, fontWeight = FontWeight.Bold, color = TEXT)
            Spacer(Modifier.height(8.dp))
            Text(sub, fontSize = 16.sp, color = MUTED)
            Spacer(Modifier.height(32.dp))
            content()
        }
    }
}

@Composable
private fun ErrorText(error: String?) {
    if (error != null) {
        Spacer(Modifier.height(16.dp))
        Text(error, color = ERROR, fontSize = 15.sp)
    }
}

@Composable
fun SetupScreen(model: AppModel, error: String?) {
    var address by rememberSaveable { mutableStateOf(model.prefs.server?.removePrefix("http://") ?: "") }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    val go = { if (!model.busy) model.connect(address) }
    Panel("Valo", "Enter your media server's address, like 192.168.1.20:8080 or nas.local:8080.") {
        TvTextField(
            address, { address = it }, "Server address",
            keyboard = KeyboardType.Uri, imeAction = ImeAction.Go, onDone = go,
            modifier = Modifier.focusRequester(first),
        )
        Spacer(Modifier.height(24.dp))
        TvButton(if (model.busy) "Connecting…" else "Connect", go, primary = true, enabled = !model.busy)
        ErrorText(error)
    }
}

@Composable
fun LoginScreen(model: AppModel, error: String?) {
    var user by rememberSaveable { mutableStateOf(model.prefs.user ?: "") }
    var password by rememberSaveable { mutableStateOf("") }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    val go = { if (!model.busy) model.login(user, password) }
    Panel("Sign in", "to ${model.client?.name ?: "the server"}") {
        TvTextField(user, { user = it }, "Username", modifier = Modifier.focusRequester(first))
        Spacer(Modifier.height(16.dp))
        TvTextField(password, { password = it }, "Password", password = true, imeAction = ImeAction.Done, onDone = go)
        Spacer(Modifier.height(24.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TvButton(if (model.busy) "Signing in…" else "Sign in", go, primary = true, enabled = !model.busy)
            TvButton("Change server", model::changeServer)
        }
        ErrorText(error)
    }
}
