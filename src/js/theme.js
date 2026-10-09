// Theme selection. "auto" follows the Windows light/dark setting live.

const dark = window.matchMedia("(prefers-color-scheme: dark)");
let choice = "auto";

function resolve() {
  return choice === "auto" ? (dark.matches ? "dark" : "light") : choice;
}

function apply() {
  document.documentElement.dataset.theme = resolve();
}

export function setTheme(name) {
  choice = name;
  apply();
}

dark.addEventListener("change", () => {
  if (choice === "auto") apply();
});
