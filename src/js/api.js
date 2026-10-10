// Thin wrapper over the Tauri bridge. Everything native goes through here,
// so the rest of the UI never touches window.__TAURI__ directly.

const tauri = window.__TAURI__;
const invoke = (cmd, args) => tauri.core.invoke(cmd, args);

/**
 * Run the first call of a window, retrying for a few seconds if the native
 * side is not ready yet (a window can load faster than the app starts).
 */
export async function whenReady(call, { tries = 30, delay = 150 } = {}) {
  for (let i = 1; ; i++) {
    try {
      return await call();
    } catch (error) {
      if (i >= tries) throw error;
      await new Promise((r) => setTimeout(r, delay));
    }
  }
}
const listen = (event, fn) => tauri.event.listen(event, (e) => fn(e.payload));

export const timer = {
  getState: () => invoke("get_state"),
  toggle: () => invoke("timer_toggle"),
  reset: () => invoke("timer_reset"),
  skip: () => invoke("timer_skip"),
  setMode: (mode) => invoke("timer_set_mode", { mode }),
  setTask: (task) => invoke("timer_set_task", { task }),
  resolveResume: (resume) => invoke("resolve_resume", { resume }),
  onState: (fn) => listen("timer://state", fn),
  onFinished: (fn) => listen("timer://finished", fn),
};

export const settingsApi = {
  get: () => invoke("get_settings"),
  update: (settings) => invoke("update_settings", { settings }),
  reset: () => invoke("reset_settings"),
  setGlobalShortcut: (shortcut) => invoke("set_global_shortcut", { shortcut }),
  setLaunchAtStartup: (enabled) => invoke("set_launch_at_startup", { enabled }),
  onChanged: (fn) => listen("settings://changed", fn),
};

export const history = {
  list: (before, limit = 200) => invoke("list_sessions", { before: before ?? null, limit }),
  remove: (id) => invoke("delete_session", { id }),
  focusTotals: (ranges) => invoke("focus_totals", { ranges }),
  recentTasks: (limit = 8) => invoke("recent_tasks", { limit }),
  weekGoals: () => invoke("week_goals"),
  firstSessionAt: () => invoke("first_session_at"),
  exportCsv: (contents, fileName) => invoke("export_csv", { contents, fileName }),
  exportBackup: (fileName) => invoke("export_backup", { fileName }),
  importBackup: () => invoke("import_backup"),
  onChanged: (fn) => listen("history://changed", fn),
};

export const sounds = {
  list: () => invoke("list_sounds"),
  preview: (id, volume) => invoke("preview_sound", { id, volume }),
};

export const app = {
  ready: () => invoke("app_ready"),
  toggleView: () => invoke("toggle_view"),
  setLanguage: (lang) => invoke("set_language", { lang }),
};

const current = () => tauri.window.getCurrentWindow();

export const appWindow = {
  minimize: () => current().minimize(),
  toggleMaximize: () => current().toggleMaximize(),
  isMaximized: () => current().isMaximized(),
  close: () => current().close(),
  startDragging: () => current().startDragging(),
  startResizeDragging: (direction) => current().startResizeDragging(direction),
  onResized: (fn) => current().onResized(fn),
};
