// Thin wrapper over the Tauri bridge. Everything native goes through here,
// so the rest of the UI never touches window.__TAURI__ directly.

const tauri = window.__TAURI__;
const invoke = (cmd, args) => tauri.core.invoke(cmd, args);
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

export const app = {
  ready: () => invoke("app_ready"),
  setLanguage: (lang) => invoke("set_language", { lang }),
};

const current = () => tauri.window.getCurrentWindow();

export const appWindow = {
  minimize: () => current().minimize(),
  toggleMaximize: () => current().toggleMaximize(),
  isMaximized: () => current().isMaximized(),
  close: () => current().close(),
  setAlwaysOnTop: (on) => current().setAlwaysOnTop(on),
  onResized: (fn) => current().onResized(fn),
};
