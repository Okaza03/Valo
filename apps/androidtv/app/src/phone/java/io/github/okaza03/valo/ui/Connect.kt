// The screens before the library: loading, picking a server, signing in, and
// the password an admin asked for.
package io.github.okaza03.valo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.PasswordState

@Composable
fun LoadingScreen(message: String) {
    Box(Modifier.fillMaxSize().background(BG), contentAlignment = Alignment.Center) {
        Text(message, color = MUTED, fontSize = 16.sp, modifier = Modifier.padding(PAD))
    }
}

/** A centered column, like a dialog. */
@Composable
private fun Form(title: String, sub: String, content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().imePadding().verticalScroll(rememberScrollState()).padding(24.dp), contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 480.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(title, fontSize = 30.sp, fontWeight = FontWeight.Bold, color = TEXT)
            Text(sub, fontSize = 15.sp, color = MUTED)
            Spacer(Modifier.height(4.dp))
            content()
        }
    }
}

@Composable
private fun Input(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    keyboard: KeyboardType = KeyboardType.Text,
    password: Boolean = false,
    action: ImeAction = ImeAction.Next,
    onDone: (() -> Unit)? = null,
) {
    OutlinedTextField(
        value, onChange, Modifier.fillMaxWidth(),
        label = { Text(label) },
        singleLine = true,
        visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else keyboard, imeAction = action, autoCorrectEnabled = false),
        keyboardActions = if (onDone != null) KeyboardActions(onDone = { onDone() }, onGo = { onDone() }) else KeyboardActions.Default,
    )
}

@Composable
fun SetupScreen(model: AppModel, error: String?) {
    var address by rememberSaveable { mutableStateOf(model.prefs.server?.removePrefix("http://") ?: "") }
    val go = { if (!model.busy) model.connect(address) }
    Form("Valo", "Enter your media server's address, like 192.168.1.20:8080 or nas.local:8080.") {
        Input(address, { address = it }, "Server address", KeyboardType.Uri, action = ImeAction.Go, onDone = go)
        Btn(if (model.busy) "Connecting…" else "Connect", go, Modifier.fillMaxWidth(), primary = true, enabled = !model.busy)
        error?.let { Text(it, color = ERROR, fontSize = 14.sp) }
    }
}

@Composable
fun LoginScreen(model: AppModel, error: String?) {
    var user by rememberSaveable { mutableStateOf(model.prefs.user ?: "") }
    var password by rememberSaveable { mutableStateOf("") }
    val go = { if (!model.busy) model.login(user, password) }
    Form("Sign in", "to ${model.client?.name ?: "the server"}") {
        Input(user, { user = it }, "Username")
        Input(password, { password = it }, "Password", password = true, action = ImeAction.Done, onDone = go)
        Btn(if (model.busy) "Signing in…" else "Sign in", go, Modifier.fillMaxWidth(), primary = true, enabled = !model.busy)
        Btn("Change server", model::changeServer, Modifier.fillMaxWidth())
        error?.let { Text(it, color = ERROR, fontSize = 14.sp) }
    }
}

/** Shown each time the app opens while an admin asks for a new password. Later closes it until next time. */
@Composable
fun PasswordPrompt(model: AppModel) {
    val editing = model.password as? PasswordState.Editing
    var current by rememberSaveable { mutableStateOf("") }
    var new by rememberSaveable { mutableStateOf("") }
    var again by rememberSaveable { mutableStateOf("") }
    val ready = current.isNotEmpty() && new.isNotEmpty() && new == again && editing?.busy != true
    AlertDialog(
        onDismissRequest = model::closePassword,
        title = { Text(if (editing == null) "Password changed" else "Change your password") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (editing == null) {
                    Text("Use it the next time you sign in. Your other devices were signed out.", color = MUTED)
                } else {
                    Text(
                        "An admin of ${model.client?.name ?: "your server"} asked you to pick a new password. " +
                            "Until you do, this shows each time you open Valo.",
                        color = MUTED, fontSize = 14.sp,
                    )
                    Input(current, { current = it }, "Current password", password = true)
                    Input(new, { new = it }, "New password", password = true)
                    Input(again, { again = it }, "New password again", password = true, action = ImeAction.Done, onDone = {
                        if (ready) model.changePassword(current, new)
                    })
                    if (again.isNotEmpty() && new != again) Text("The new passwords don't match.", color = ERROR, fontSize = 14.sp)
                    editing.error?.let { Text(it, color = ERROR, fontSize = 14.sp) }
                }
            }
        },
        confirmButton = {
            if (editing == null) {
                TextButton(model::closePassword) { Text("Close") }
            } else {
                TextButton({ model.changePassword(current, new) }, enabled = ready) {
                    Text(if (editing.busy) "Saving…" else "Change password")
                }
            }
        },
        dismissButton = { if (editing != null) TextButton(model::closePassword) { Text("Later") } },
    )
}
