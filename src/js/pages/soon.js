// Placeholder for pages built in later stages.

import { translate } from "../i18n.js";
import { renderIcons } from "../icons.js";

export function create(icon) {
  return function mount(root) {
    root.innerHTML = `
      <div class="empty">
        <span data-icon="${icon}"></span>
        <h2 data-i18n="soon.title"></h2>
        <p data-i18n="soon.body"></p>
      </div>`;
    translate(root);
    renderIcons(root);
    return () => {};
  };
}
