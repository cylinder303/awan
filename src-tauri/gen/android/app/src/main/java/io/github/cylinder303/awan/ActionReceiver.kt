package io.github.cylinder303.awan

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** The session notification's buttons. They reach the running app's page. */
class ActionReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    val action = intent.getStringExtra(SessionNotification.EXTRA_ACTION) ?: return
    val activity = MainActivity.live
    if (activity != null) {
      activity.runOnUiThread { activity.dispatchAction(action) }
    } else {
      SessionNotification.showClosed(context)
    }
  }
}
