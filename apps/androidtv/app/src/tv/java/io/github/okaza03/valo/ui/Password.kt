// "Change your password", shown each time the app opens while an admin asks
// for a new one (`must_change_password` from `GET /api/me`). Later closes it
// until the next start (the desktop app's browse/password.rs).
package io.github.okaza03.valo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import io.github.okaza03.valo.AppModel
import io.github.okaza03.valo.PasswordState

@Composable
fun PasswordPrompt(model: AppModel) {
    val state = model.password
    val first = remember { FocusRequester() }
    LaunchedEffect(state is PasswordState.Changed) { first.focusSoon() }
    var current by rememberSaveable { mutableStateOf("") }
    var new by rememberSaveable { mutableStateOf("") }
    var again by rememberSaveable { mutableStateOf("") }
    val editing = state as? PasswordState.Editing

    Modal(560.dp, model::closePassword, footer = {
        if (editing == null) {
            TvButton("Close", model::closePassword, Modifier.focusRequester(first), primary = true)
            return@Modal
        }
        val ready = current.isNotEmpty() && new.isNotEmpty() && new == again && !editing.busy
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TvButton(if (editing.busy) "Saving…" else "Change password", { model.changePassword(current, new) }, primary = true, enabled = ready)
            TvButton("Later", model::closePassword)
        }
    }) {
        Column(Modifier.padding(horizontal = 32.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (editing == null) {
                MonoText("Done", 12.sp, OK)
                Text("Password changed", fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
                Text("Use it the next time you sign in. Your other devices were signed out.", fontSize = 15.sp, color = MUTED)
                return@Column
            }
            MonoText("Your account", 12.sp, TEAL)
            Text("Change your password", fontSize = 28.sp, fontWeight = FontWeight.Bold, color = TEXT)
            Text(
                "An admin of ${model.client?.name ?: "your server"} asked you to pick a new password. " +
                    "Until you do, this shows each time you open Valo.",
                fontSize = 15.sp, color = MUTED,
            )
            Spacer(Modifier.height(10.dp))
            TvTextField(current, { current = it }, "Current password", Modifier.fillMaxWidth().focusRequester(first), password = true)
            TvTextField(new, { new = it }, "New password", Modifier.fillMaxWidth(), password = true)
            TvTextField(
                again, { again = it }, "New password again", Modifier.fillMaxWidth(), password = true, imeAction = ImeAction.Done,
                onDone = { if (current.isNotEmpty() && new.isNotEmpty() && new == again) model.changePassword(current, new) },
            )
            if (again.isNotEmpty() && new != again) Text("The new passwords don't match.", fontSize = 14.sp, color = ERROR)
            editing.error?.let { Text(it, fontSize = 14.sp, color = ERROR) }
        }
    }
}
