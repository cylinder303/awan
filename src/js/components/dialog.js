// Small modal dialog. Resolves with the value of the chosen action.

/**
 * @param {object} opts
 * @param {string} opts.title
 * @param {string} opts.body
 * @param {{label: string, value: any, kind?: "primary"|"danger"}[]} opts.actions
 * @param {boolean} [opts.dismissible=true] Escape / backdrop click resolves with null.
 */
export function openDialog({ title, body, actions, dismissible = true }) {
  return new Promise((resolve) => {
    const backdrop = document.createElement("div");
    backdrop.className = "dialog-backdrop";
    backdrop.innerHTML = `
      <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
        <h2 id="dialog-title"></h2>
        <p></p>
        <div class="dialog__actions"></div>
      </div>`;
    backdrop.querySelector("h2").textContent = title;
    backdrop.querySelector("p").textContent = body;

    const row = backdrop.querySelector(".dialog__actions");
    const buttons = actions.map((action) => {
      const btn = document.createElement("button");
      btn.className = "btn" + (action.kind ? ` btn--${action.kind}` : "");
      btn.textContent = action.label;
      btn.addEventListener("click", () => close(action.value));
      row.append(btn);
      return btn;
    });

    const previousFocus = document.activeElement;

    function onKey(e) {
      if (e.key === "Escape" && dismissible) close(null);
      e.stopPropagation();
    }

    function close(value) {
      document.removeEventListener("keydown", onKey, true);
      delete document.body.dataset.modal;
      backdrop.remove();
      previousFocus?.focus?.();
      resolve(value);
    }

    backdrop.addEventListener("click", (e) => {
      if (e.target === backdrop && dismissible) close(null);
    });
    document.addEventListener("keydown", onKey, true);
    document.body.dataset.modal = "open";
    document.body.append(backdrop);
    (buttons.find((_, i) => actions[i].kind === "primary") || buttons[0])?.focus();
  });
}
