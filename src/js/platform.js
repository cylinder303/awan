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
