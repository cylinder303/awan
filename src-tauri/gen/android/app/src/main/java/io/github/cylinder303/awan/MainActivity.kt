package io.github.cylinder303.awan

import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.util.Locale

class MainActivity : TauriActivity() {
  /** Space taken by the status bar, navigation bar, camera cutout and
   *  keyboard, in CSS pixels, as JSON. Read by src/js/platform.js. */
  @Volatile private var insets = "{\"top\":0,\"right\":0,\"bottom\":0,\"left\":0}"

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(InsetsBridge(), "AwanAndroid")
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

  private inner class InsetsBridge {
    @JavascriptInterface
    fun insets(): String = this@MainActivity.insets
  }
}
