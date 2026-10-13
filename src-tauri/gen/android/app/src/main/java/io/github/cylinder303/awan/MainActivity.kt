package io.github.cylinder303.awan

import android.content.pm.PackageManager
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.util.Locale

/** What the page reaches through `window.AwanAndroid` (see src/js/platform.js). */
class MainActivity : TauriActivity() {
  /** Space taken by the status bar, navigation bar, camera cutout and
   *  keyboard, in CSS pixels, as JSON. */
  @Volatile private var insets = "{\"top\":0,\"right\":0,\"bottom\":0,\"left\":0}"
  private var webView: WebView? = null
  private var player: MediaPlayer? = null

  /** Android 17 asks before an app talks to devices on the local network
   *  (here: the computer it syncs with). */
  private val askLocalNetwork = registerForActivityResult(ActivityResultContracts.RequestPermission()) {
    webView?.evaluateJavascript("window.dispatchEvent(new Event('awan:local-network'))", null)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
    webView.addJavascriptInterface(Bridge(), "AwanAndroid")
    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, windowInsets ->
      val bars = windowInsets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val keyboard = windowInsets.getInsets(WindowInsetsCompat.Type.ime())
      val density = view.resources.displayMetrics.density
      // Locale.ROOT: plain digits and a dot, whatever the phone's language.
      insets = String.format(
        Locale.ROOT,
        "{\"top\":%.1f,\"right\":%.1f,\"bottom\":%.1f,\"left\":%.1f}",
        bars.top / density,
        bars.right / density,
        maxOf(bars.bottom, keyboard.bottom) / density,
        bars.left / density,
      )
      webView.evaluateJavascript("window.dispatchEvent(new Event('awan:insets'))", null)
      // Let the WebView see them too (env(safe-area-inset-*) on newer versions).
      ViewCompat.onApplyWindowInsets(view, windowInsets)
    }
  }

  override fun onDestroy() {
    player?.release()
    player = null
    super.onDestroy()
  }

  private fun localNetworkGranted(): Boolean =
    Build.VERSION.SDK_INT < LOCAL_NETWORK_SDK ||
      ContextCompat.checkSelfPermission(this, LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED

  /** Play an alert sound from res/raw on the notification stream, so the
   *  preview sounds like the alert itself. */
  private fun play(name: String, volume: Float) {
    player?.release()
    player = null
    val id = resources.getIdentifier(name, "raw", packageName)
    if (id == 0 || volume <= 0f) return
    val attributes = AudioAttributes.Builder()
      .setUsage(AudioAttributes.USAGE_NOTIFICATION)
      .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
      .build()
    val session = (getSystemService(AUDIO_SERVICE) as AudioManager).generateAudioSessionId()
    player = MediaPlayer.create(this, id, attributes, session)?.apply {
      setVolume(volume, volume)
      setOnCompletionListener {
        it.release()
        if (player === it) player = null
      }
      start()
    }
  }

  private inner class Bridge {
    @JavascriptInterface
    fun insets(): String = this@MainActivity.insets

    @JavascriptInterface
    fun playSound(name: String, volume: Float) {
      runOnUiThread { play(name, volume.coerceIn(0f, 1f)) }
    }

    /** The name the phone's owner gave it, or its model. */
    @JavascriptInterface
    fun deviceName(): String =
      Settings.Global.getString(contentResolver, "device_name")?.takeIf { it.isNotBlank() }
        ?: Build.MODEL

    /** "granted", or "ask" when the system prompt has to be shown first. */
    @JavascriptInterface
    fun localNetwork(): String = if (localNetworkGranted()) "granted" else "ask"

    @JavascriptInterface
    fun requestLocalNetwork() {
      runOnUiThread {
        if (localNetworkGranted()) {
          webView?.evaluateJavascript("window.dispatchEvent(new Event('awan:local-network'))", null)
        } else {
          askLocalNetwork.launch(LOCAL_NETWORK)
        }
      }
    }
  }

  private companion object {
    const val LOCAL_NETWORK = "android.permission.ACCESS_LOCAL_NETWORK"
    /** Android 17, where the permission starts to apply. */
    const val LOCAL_NETWORK_SDK = 37
  }
}
