// Phone or computer. On a phone the page has no title bar and leaves room
// for the status bar, the navigation bar, a camera cutout and the keyboard.

export const isMobile = /Android|iPhone|iPad/i.test(navigator.userAgent);

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
