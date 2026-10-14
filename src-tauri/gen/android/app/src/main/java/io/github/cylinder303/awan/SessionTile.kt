package io.github.cylinder303.awan

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/** A Quick Settings button that starts or pauses the timer in one tap. */
class SessionTile : TileService() {
  override fun onStartListening() {
    val tile = qsTile ?: return
    val state = SessionNotification.saved(this)
    tile.state = if (state?.optString("status") == "running") Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
    if (Build.VERSION.SDK_INT >= 29) tile.subtitle = state?.optString("tile")?.takeIf { it.isNotEmpty() }
    tile.updateTile()
  }

  override fun onClick() {
    val activity = MainActivity.live
    if (activity != null) {
      activity.runOnUiThread { activity.dispatchAction("toggle") }
      return
    }
    // The app is not running: open it, and it starts the timer once ready.
    val intent = Intent(this, MainActivity::class.java)
      .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      .putExtra(SessionNotification.EXTRA_ACTION, "toggle")
    if (Build.VERSION.SDK_INT >= 34) {
      startActivityAndCollapse(PendingIntent.getActivity(this, 3, intent, PendingIntent.FLAG_IMMUTABLE))
    } else {
      @Suppress("DEPRECATION")
      startActivityAndCollapse(intent)
    }
  }
}
