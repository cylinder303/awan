// Settings shared by every view. The backend validates and stores them;
// this module keeps the latest copy and notifies subscribers.

import { settingsApi } from "./api.js";

let current = null;
let latest = 0; // id of the most recent local update
let pending = 0; // local updates still waiting for the backend
const subscribers = new Set();

function publish(next) {
  if (JSON.stringify(next) === JSON.stringify(current)) return;
  current = next;
  subscribers.forEach((fn) => fn(current));
}

export const settings = {
  get: () => current,

  async load() {
    publish(await settingsApi.get());
    // Changes made in another window (e.g. mini mode) arrive here too.
    // Our own echoes are ignored while a local change is still in flight.
    await settingsApi.onChanged((next) => pending === 0 && publish(next));
    return current;
  },

  /** Merge `patch` into the settings, apply it at once, then save it. */
  async update(patch) {
    const id = ++latest;
    pending++;
    publish({ ...current, ...patch });
    try {
      const saved = await settingsApi.update(current);
      // A newer change may already be on its way; let that one win.
      if (id === latest) publish(saved);
    } finally {
      pending--;
    }
  },

  /** Take settings returned by a backend command (e.g. the shortcut editor). */
  adopt(next) {
    latest++;
    publish(next);
  },

  async reset() {
    latest++;
    publish(await settingsApi.reset());
  },

  subscribe(fn) {
    subscribers.add(fn);
    if (current) fn(current);
    return () => subscribers.delete(fn);
  },
};
