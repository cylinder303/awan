// Set the app version everywhere it is written down.
//   npm run version -- 1.2.0
// Then commit, and publish a release tagged v1.2.0.

import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("Usage: npm run version -- <major.minor.patch>   e.g. 1.2.0");
  process.exit(1);
}

const edits = [
  ["package.json", (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`)],
  ["src-tauri/tauri.conf.json", (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`)],
  ["src-tauri/Cargo.toml", (s) => s.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`)],
];
// package-lock.json repeats the version for the root package.
edits.push([
  "package-lock.json",
  (s) => {
    const lock = JSON.parse(s);
    lock.version = version;
    if (lock.packages?.[""]) lock.packages[""].version = version;
    return JSON.stringify(lock, null, 2) + "\n";
  },
]);

for (const [file, edit] of edits) {
  writeFileSync(file, edit(readFileSync(file, "utf8")));
  console.log(`${file} -> ${version}`);
}
console.log("Cargo.lock updates on the next build.");
