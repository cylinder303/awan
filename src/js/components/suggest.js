// Suggestion list under a text input (used for recent task names).

/**
 * @param {HTMLInputElement} input
 * @param {() => Promise<string[]>} source returns candidates, best first
 * @param {(value: string) => void} onPick
 */
export function attachSuggestions(input, source, onPick) {
  const list = document.createElement("ul");
  list.className = "suggest";
  list.setAttribute("role", "listbox");
  list.id = `suggest-${Math.random().toString(36).slice(2)}`;
  list.hidden = true;
  input.setAttribute("aria-controls", list.id);
  input.setAttribute("aria-autocomplete", "list");
  input.insertAdjacentElement("afterend", list);

  let items = [];
  let active = -1;

  async function refresh() {
    const query = input.value.trim().toLowerCase();
    const all = await source();
    items = all.filter((t) => t.toLowerCase() !== query && t.toLowerCase().includes(query)).slice(0, 5);
    active = -1;
    render();
  }

  function render() {
    list.innerHTML = "";
    items.forEach((text, i) => {
      const li = document.createElement("li");
      li.className = "suggest__item";
      li.setAttribute("role", "option");
      li.setAttribute("aria-selected", String(i === active));
      li.textContent = text;
      // mousedown fires before the input loses focus.
      li.addEventListener("mousedown", (e) => {
        e.preventDefault();
        pick(text);
      });
      list.append(li);
    });
    list.hidden = items.length === 0 || document.activeElement !== input;
  }

  function pick(text) {
    input.value = text;
    items = [];
    render();
    onPick(text);
  }

  input.addEventListener("focus", refresh);
  input.addEventListener("input", refresh);
  input.addEventListener("blur", () => (list.hidden = true));
  input.addEventListener("keydown", (e) => {
    if (list.hidden) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      active = (active + step + items.length) % items.length;
      render();
    } else if (e.key === "Enter" && active >= 0) {
      e.preventDefault();
      e.stopPropagation();
      pick(items[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      items = [];
      render();
    }
  });
}
