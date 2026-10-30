# Awan

A calm, offline Pomodoro timer for Windows and Android: Tauri 2 (Rust) with a
plain HTML/CSS/JS front end (no framework, no bundler) and SQLite storage.
Build steps, layout and how-tos for people are in `CONTRIBUTING.md`.

## Working with the owner

- Reply, explain and give instructions in Arabic. Code, comments, file names
  and commit messages are in English.
- The owner does not read code. Explain changes in a few plain Arabic lines,
  and give short, clear manual test steps on Windows.
- Before changing anything: if a request is not technically possible, or a
  better alternative exists, say so first.
- At the end of each stage, list what was tested automatically and what the
  owner should test by hand on Windows.
- Work on a new branch. Changes reach `main` through a pull request the owner
  reviews; push to `main` directly only with explicit permission.
- Never publish a release without explicit permission (for example «انشر»).
- Alert sounds are natural sounds only: no music, no instruments, no melodies.
- Never change the identifier `io.github.cylinder303.awan`: it names the
  owner's data folder (`%APPDATA%\io.github.cylinder303.awan\awan.db`).

## Layout

- `src/`: the UI. `index.html` (main window) and `mini.html` (mini timer);
  `js/pages/` (timer, history, goals, settings, sync section, font picker);
  `js/api.js` is the only module that calls Tauri; `locales/ar.js` and
  `locales/en.js`; `styles/themes.css` holds every theme; `styles/fonts.css`
  and `js/font-list.js` the bundled fonts.
- `src-tauri/src/`: the native side. `timer.rs` (engine; the timer runs here,
  not in the UI), `clock.rs` (clock without sleep time), `power.rs`,
  `settings.rs`, `db/` (SQLite, migrations), `streak.rs`, `goals.rs`,
  `sync/` (phone-computer sync over Wi-Fi), `fonts.rs`, `sound.rs`, `tray.rs`,
  `display.rs` (windows and mini mode), `shortcuts.rs`, `backup.rs`,
  `alerts.rs` (Android), `commands/` (everything the UI can call), `lib.rs`.
- `src-tauri/gen/android/`: the Android project.
- `scripts/`: icon, sound and test-font generators, version bump, Windows
  smoke and installer tests.
- `tests/`: UI tests (`node --test`).

## Commands

```
npm ci                         # build tools
npm test                       # UI tests
cd src-tauri && cargo test     # Rust tests
npm run dev                    # run the app (see the data warning below)
npm run build                  # NSIS installer in src-tauri/target/release/bundle/nsis
npm run icons                  # icons from scripts/icon.svg and tray-icon.svg
npm run version -- x.y.z       # bump the version everywhere
```

A development run with the real identifier opens the owner's real data. Test
with another identifier, which gets its own data folder:
`TAURI_CONFIG={"identifier":"io.github.cylinder303.awan.dev"}`.

## Rules in the code

- Every UI string goes into both locale files; a test checks they match.
- Settings are one JSON document (`settings.rs`): every field has a default
  and passes `sanitized()`. Fields in `Shared` sync between devices; the rest
  (volume, windows, shortcuts, fonts) stay per device.
- Database changes are new migrations in `db/migrations.rs`
  (`PRAGMA user_version`); never edit an existing one.
- Numbers always use Latin digits, in both languages.
- A theme is one block in `themes.css`; the picker finds it.

## CI and releases

- `.github/workflows/ci.yml` runs on every push: on Windows the tests, the
  installer build, an install/uninstall test and a smoke test; on Linux the
  Android APK.
- Releases (only with permission): `npm run version -- x.y.z`, merge to
  `main`, then Actions → Release → Run workflow with tag `vx.y.z` (or push
  the tag). The workflow checks the tag matches the version, builds, tests and
  publishes with `.github/release-notes.md`.
