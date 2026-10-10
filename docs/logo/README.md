# Logo

A ring three-quarters full: a session in progress.

| | App icon | Tray icon |
| --- | --- | --- |
| **Current: terracotta** | <img src="../../scripts/icon.svg" width="96" alt="Terracotta app icon"> | <img src="../../scripts/tray-icon.svg" width="32" alt="Terracotta tray icon"> |
| Alternate: mint | <img src="mint/icon.svg" width="96" alt="Mint app icon"> | <img src="mint/tray-icon.svg" width="32" alt="Mint tray icon"> |

Terracotta matches the Focus color the app opens with. Mint (the Break
color of the Dark theme) is kept here, ready to use.

## Where the logo lives

| Source | Used for | Regenerate with |
| --- | --- | --- |
| `scripts/icon.svg` | window, taskbar, Start menu, installer icon (`src-tauri/icons/`) | `npm run icons` |
| `scripts/tray-icon.svg` | system tray, one PNG per display scale (`src-tauri/icons/tray/`) | `npm run icons` |
| `scripts/icon.svg` | installer welcome and page header (`src-tauri/installer/*.bmp`) | `node scripts/installer-art.mjs` |

`installer-art.mjs` needs `npm install --no-save playwright-core@1` first. On
Windows it uses Microsoft Edge; elsewhere set `CHROME_PATH` to Chrome or
Chromium.

## Switch to the mint logo

Everything is pre-rendered, so no browser is needed:

```bash
cp docs/logo/mint/icon.svg docs/logo/mint/tray-icon.svg scripts/
cp docs/logo/mint/sidebar.bmp docs/logo/mint/header.bmp src-tauri/installer/
npm run icons
```

Then commit; the next build uses the new logo. To switch back, run
`git checkout <commit> -- scripts src-tauri/icons src-tauri/installer` with a
commit from before the switch.
