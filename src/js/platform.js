// Phone or computer. On a phone the page has no title bar and leaves room
// for the status bar, the navigation bar, a camera cutout and the keyboard.

export const isMobile = /Android|iPhone|iPad/i.test(navigator.userAgent);

/** The phone's name, for the computer's list of linked phones. */
export function deviceName() {
  try {
    return window.AwanAndroid?.deviceName() || "";
  } catch {
    return "";
  }
}

/**
 * Android 17 asks before an app reaches devices on the local network.
 * Resolves true once allowed (at once on older phones and on computers).
 */
export function ensureLocalNetwork() {
  const bridge = window.AwanAndroid;
  if (!bridge?.localNetwork || bridge.localNetwork() === "granted") return Promise.resolve(true);
  return new Promise((resolve) => {
    const done = () => {
      window.removeEventListener("awan:local-network", done);
      resolve(bridge.localNetwork() === "granted");
    };
    window.addEventListener("awan:local-network", done);
    bridge.requestLocalNetwork();
  });
}

/**
 * Android: the back button and the actions coming from the session
 * notification and the Quick Settings tile ("toggle", "skip").
 * `back()` returns true when it handled the press (else the app goes away).
 */
export function initAndroidActions({ back, action }) {
  const bridge = window.AwanAndroid;
  if (!bridge?.takePendingAction) return;
  window.__awanBack = back;
  window.__awanAction = action;
  const pending = bridge.takePendingAction();
  if (pending) action(pending);
}

/** Show the running or paused session in a notification and the tile. */
export function showSession(state) {
  try {
    window.AwanAndroid?.session?.(JSON.stringify(state));
  } catch {
    // Older app without the bridge method: nothing to show.
  }
}

/** Mark the page with its platform and keep the system insets current. */
export function initPlatform() {
  const root = document.documentElement;
  root.dataset.platform = isMobile ? "mobile" : "desktop";

  // The Android app reports the insets itself (MainActivity.kt): older
  // WebViews give wrong env(safe-area-inset-*) values, which stay the fallback.
  const bridge = window.AwanAndroid;
  if (!bridge) return;
  const apply = () => {
    let insets;
    try {
      insets = JSON.parse(bridge.insets());
    } catch {
      return;
    }
    for (const side of ["top", "right", "bottom", "left"]) {
      root.style.setProperty(`--inset-${side}`, `${Number(insets[side]) || 0}px`);
    }
  };
  apply();
  window.addEventListener("awan:insets", apply);
}
