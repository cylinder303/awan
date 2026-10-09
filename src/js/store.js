// Latest timer state from the backend, shared by every view.

let view = null;
let receivedAt = 0;
const subscribers = new Set();
const finishedSubscribers = new Set();

export const store = {
  get: () => view,

  /** Milliseconds since the current state arrived (for smooth animation). */
  age: () => performance.now() - receivedAt,

  set(next) {
    view = next;
    receivedAt = performance.now();
    subscribers.forEach((fn) => fn(view));
  },

  subscribe(fn) {
    subscribers.add(fn);
    if (view) fn(view);
    return () => subscribers.delete(fn);
  },

  finished(event) {
    finishedSubscribers.forEach((fn) => fn(event));
  },

  onFinished(fn) {
    finishedSubscribers.add(fn);
    return () => finishedSubscribers.delete(fn);
  },
};
