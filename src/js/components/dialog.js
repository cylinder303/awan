// Small modal dialog. Resolves with the value of the chosen action.

/**
 * @param {object} opts
 * @param {string} opts.title
 * @param {string} opts.body
 * @param {{label: string, value: any, kind?: "primary"|"danger", run?: () => Promise<boolean>}[]} opts.actions
 *   An action with `run` closes the dialog only if it resolves to true.
 * @param {boolean} [opts.dismissible=true] Escape / backdrop click resolves with null.
 * @param {Element} [opts.content] Shown under the body.
 * @param {(close: (value: any) => void) => void} [opts.onOpen] Gets a way to close it from outside.
 */
export function openDialog({ title, body, actions, dismissible = true, content, onOpen }) {
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
    if (content) backdrop.querySelector("p").after(content);

    const row = backdrop.querySelector(".dialog__actions");
    const buttons = actions.map((action) => {
      const btn = document.createElement("button");
      btn.className = "btn" + (action.kind ? ` btn--${action.kind}` : "");
      btn.textContent = action.label;
      btn.addEventListener("click", async () => {
        if (!action.run) return close(action.value);
        btn.disabled = true;
        try {
          if (await action.run()) close(action.value);
        } finally {
          btn.disabled = false;
        }
      });
      row.append(btn);
      return btn;
    });

    const previousFocus = document.activeElement;

    function onKey(e) {
      if (e.key === "Escape" && dismissible) close(null);
      e.stopPropagation();
    }

    let closed = false;
    function close(value) {
      if (closed) return;
      closed = true;
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
    if (!content) (buttons.find((_, i) => actions[i].kind === "primary") || buttons[0])?.focus();
    onOpen?.(close);
  });
}
