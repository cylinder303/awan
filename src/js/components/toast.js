// Brief message at the bottom of the window.

let current = null;

export function toast(message, { error = false } = {}) {
  current?.remove();
  const el = document.createElement("div");
  el.className = "toast" + (error ? " toast--error" : "");
  el.setAttribute("role", "status");
  el.textContent = message;
  document.body.append(el);
  current = el;
  setTimeout(() => {
    el.classList.add("is-leaving");
    setTimeout(() => el.remove(), 300);
  }, 2800);
}
