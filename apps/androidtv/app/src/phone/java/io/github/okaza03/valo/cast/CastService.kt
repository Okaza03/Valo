// Keeps the phone streaming to the TV with its screen off: the video goes
// through the phone (CastProxy), so without this Android would put the app to
// sleep and the TV would stop a few minutes later.
package io.github.okaza03.valo.cast

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import io.github.okaza03.valo.R

class CastService : Service() {
    private var wifi: WifiManager.WifiLock? = null
    private var wake: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        @Suppress("DEPRECATION")
        val mode = if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY else WifiManager.WIFI_MODE_FULL_HIGH_PERF
        wifi = wm.createWifiLock(mode, "valo:cast").apply { acquire() }
        val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
        wake = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "valo:cast").apply { acquire() }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Android wants the notification first, whatever the reason for the call
        val n = notification()
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        } else {
            startForeground(ID, n)
        }
        when (intent?.action) {
            ACTION_TOGGLE -> caster?.togglePause()
            ACTION_STOP -> caster?.stop()
        }
        return START_NOT_STICKY
    }

    // the app was swiped away: what the TV plays comes through it, so casting ends here
    override fun onTaskRemoved(rootIntent: Intent?) {
        caster?.stop()
        stopSelf()
    }

    override fun onDestroy() {
        wifi?.takeIf { it.isHeld }?.release()
        wake?.takeIf { it.isHeld }?.release()
        super.onDestroy()
    }

    private fun action(action: String) = PendingIntent.getService(
        this, action.hashCode(), Intent(this, CastService::class.java).setAction(action),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Casting", NotificationManager.IMPORTANCE_LOW))
        }
        val c = caster
        val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        }
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_cast)
            .setContentTitle(c?.now?.title ?: "Casting")
            .setContentText(c?.device?.let { "Playing on $it. Keep this phone on the Wi-Fi." })
            .setContentIntent(open)
            .setOngoing(true)
            .setSilent(true)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .addAction(
                if (c?.playing == true) R.drawable.ic_pause else R.drawable.ic_play,
                if (c?.playing == true) "Pause" else "Play", action(ACTION_TOGGLE),
            )
            .addAction(R.drawable.ic_close, "Stop casting", action(ACTION_STOP))
            .build()
    }

    companion object {
        private const val ID = 1
        private const val CHANNEL = "casting"
        private const val ACTION_TOGGLE = "toggle"
        private const val ACTION_STOP = "stop"

        /** Who the notification's buttons talk to. */
        var caster: Caster? = null

        /** Start, or show what changed. */
        fun show(ctx: Context) {
            runCatching { ContextCompat.startForegroundService(ctx, Intent(ctx, CastService::class.java)) }
        }

        fun end(ctx: Context) {
            ctx.stopService(Intent(ctx, CastService::class.java))
        }
    }
}
