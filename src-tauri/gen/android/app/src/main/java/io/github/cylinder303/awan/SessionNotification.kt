package io.github.cylinder303.awan

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.service.quicksettings.TileService
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject

/**
 * The notification shown while a session runs or is paused: the time left
 * counts down by itself (the app does not need to run), with buttons to
 * pause or resume and to skip. The page sends the state as JSON
 * (src/js/platform.js); it is also kept for the Quick Settings tile.
 */
object SessionNotification {
  private const val CHANNEL = "session"
  private const val ID = 4711
  private const val PREFS = "awan"
  private const val KEY = "session"
  const val EXTRA_ACTION = "awan_action"

  fun saved(context: Context): JSONObject? =
    context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null)?.let {
      runCatching { JSONObject(it) }.getOrNull()
    }

  fun update(context: Context, json: String) {
    context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY, json).apply()
    val state = runCatching { JSONObject(json) }.getOrNull() ?: return
    show(context, state, closed = false)
    TileService.requestListeningState(context, ComponentName(context, SessionTile::class.java))
  }

  /** A button was pressed while the app was closed: it cannot act, so point to the app. */
  fun showClosed(context: Context) {
    saved(context)?.let { show(context, it, closed = true) }
  }

  private fun show(context: Context, state: JSONObject, closed: Boolean) {
    val manager = NotificationManagerCompat.from(context)
    val status = state.optString("status")
    if (status != "running" && status != "paused") {
      manager.cancel(ID)
      return
    }
    if (Build.VERSION.SDK_INT >= 33 &&
      ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
    ) return
    if (Build.VERSION.SDK_INT >= 26) {
      val channel = NotificationChannel(CHANNEL, state.optString("channel", "Session"), NotificationManager.IMPORTANCE_LOW)
      channel.setShowBadge(false)
      (context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).createNotificationChannel(channel)
    }
    val open = PendingIntent.getActivity(
      context, 0,
      Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
    val builder = NotificationCompat.Builder(context, CHANNEL)
      .setSmallIcon(R.drawable.ic_stat_awan)
      .setContentTitle(state.optString("title"))
      .setContentIntent(open)
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setSilent(true)
      .setCategory(NotificationCompat.CATEGORY_PROGRESS)
      .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
    val endsAt = state.optLong("endsAt")
    if (status == "running" && endsAt > System.currentTimeMillis()) {
      builder.setWhen(endsAt).setShowWhen(true).setUsesChronometer(true).setChronometerCountDown(true)
        // Gone when the session ends; the end alert takes over.
        .setTimeoutAfter(endsAt - System.currentTimeMillis())
    } else {
      builder.setShowWhen(false).setContentText(state.optString("text"))
    }
    if (closed) {
      builder.setContentText(state.optString("openHint"))
    } else {
      val toggle = if (status == "running") state.optString("pause") else state.optString("resume")
      builder.addAction(0, toggle, action(context, "toggle", 1))
      builder.addAction(0, state.optString("skip"), action(context, "skip", 2))
    }
    try {
      manager.notify(ID, builder.build())
    } catch (_: SecurityException) {
      // Notifications were turned off in the meantime.
    }
  }

  private fun action(context: Context, name: String, code: Int): PendingIntent =
    PendingIntent.getBroadcast(
      context, code,
      Intent(context, ActionReceiver::class.java).putExtra(EXTRA_ACTION, name),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
}
