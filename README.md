# FluidaOS

FluidaOS is a modular browser desktop with persistent local data, movable windows, and integrated productivity apps. Its original glass-and-gradient interface is built with TypeScript, CSS design tokens, and no proprietary assets.

## Run locally

Requires a stable Rust toolchain (Rust 1.80 or newer), Cargo, and Node.js 20 or newer. Node.js is used only to compile the browser client.

```bash
npm install
npm run dev
```

Open `http://localhost:3000`. `npm run dev` compiles the frontend and starts the Rust server with Cargo. For a production-style run use `npm start`; run the frontend build and Rust test suite with `npm test`. To run the steps separately, use `npm run build`, `cargo run -p fluida-server`, and `cargo test --workspace`. Cargo incremental compilation is disabled in development and release profiles. User content is stored under `os_storage/`, while configuration and app records live in `os_storage/.fluida/`.

## Architecture

- `src/client/core` contains bootstrap, shared state, and the API client.
- `src/client/shell` implements window lifecycle, focus, dragging, resize, and task switching.
- `src/client/apps` contains the built-in application views and safe terminal commands.
- `src/client/styles` contains design tokens, responsive layout, themes, wallpapers, and motion rules.
- `crates/fluida-server` contains the Axum HTTP server, persistent repositories, storage access, validation, session middleware, and typed errors.
- `src/shared/types` defines domain contracts used by client and server.

## Built-in features

The desktop includes draggable shortcuts, a running-app dock, resizable and maximizable windows, task switching, global app search, notifications, Quick Settings, Files, Editor, Notes with auto-save and folders, a restricted Terminal, System Monitor, Calculator history, Settings, four safe wallpapers, theme switching, and reduced-motion support.

The five additional default applications are Clock (world clocks, alarms, timer, and stopwatch state), Calendar (month navigation, local events, and agenda), Photos (a storage-backed image gallery), Archive Manager (non-executable path collections), and App Center (application metadata and enable/disable controls). They appear in the desktop, dock, and command palette like every other application.

### Restricted Terminal

The Terminal never invokes a host shell or arbitrary process. Its 20 commands operate only through the contained FluidaOS file service:

| Command | Purpose |
| --- | --- |
| `help`, `pwd`, `date`, `clear` | Help, current storage path, fixed server time, and screen clearing |
| `cd`, `ls`, `tree`, `find`, `stat` | Navigate and inspect contained paths |
| `cat`, `head`, `tail` | Read files with bounded output |
| `mkdir`, `rmdir` | Create folders or remove empty folders; recursive removal requires `--recursive` |
| `touch`, `write`, `append` | Create or update files |
| `cp`, `mv`, `rm` | Copy, move, or remove files; `rm` cannot remove folders |

Quoted arguments are supported. Commands validate argument counts, retain a session-owned working directory, cap output at 64 KiB, and reject absolute paths, traversal, null bytes, and unknown commands.

### Files

Files supports folder navigation and breadcrumbs, Up navigation, create, rename, move, copy, confirmed recursive delete, Ctrl/Command multi-selection, recursive search, name/modified/size sorting, metadata, and bounded text previews. Grid and list views persist in `Settings.filesView`; loading, empty, retry, and error states are shown in place.

### Settings

Settings is organized into Appearance, Accessibility, Desktop, Notifications, Storage, Applications, and System. Preferences include theme, accent, wallpaper, font and text scale, dock position and auto-hide, animation level, reduced motion, high contrast, focus visibility, workspace behavior, shortcut layout, clock format, notification controls, default apps, Files view, and reset-to-defaults. Nested preferences are deep-merged. The browser's `prefers-reduced-motion` choice always disables motion even if saved settings request animation.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| Ctrl/Command + K | Open global application search |
| Alt + Tab | Focus the next running window |
| Ctrl/Command + W | Close the active window |
| Ctrl/Command + S | Save the active Editor document |

## API

All endpoints return JSON. Failures use `{ "success": false, "error": { "code", "message" } }`.

| Endpoint | Purpose |
| --- | --- |
| `GET /api/fs?path=` | List a contained storage directory |
| `GET /api/fs/search`, `/meta` and `POST /api/fs/preview` | Search and inspect contained files |
| `POST /api/fs/read`, `/write`, `/mkdir`, `/rename`, `/copy` | Perform validated file operations |
| `DELETE /api/fs` | Delete an item; recursive folder deletion requires explicit confirmation |
| `GET /api/apps`, `/apps/all` | List enabled apps or all app metadata |
| `GET`, `PUT /api/settings`; `POST /api/settings/reset` | Read, update, or reset preferences |
| `GET`, `PUT /api/app-state/:app` | Persist Clock, Calendar, and Archive state |
| `GET /api/photos` | List safe image metadata from storage |
| `GET`, `POST`, `DELETE /api/notes/:id` | Search, save, and remove notes |
| `GET`, `PATCH`, `DELETE /api/notifications/:id` | Manage notification state |
| `GET /api/system` | Report storage, runtime, and app totals |
| `GET`, `PUT /api/device-control` | Read or update explicitly enabled local host controls |

Storage paths must be relative. Absolute paths, null bytes, traversal outside `os_storage`, invalid request bodies, unknown or out-of-range settings, invalid Base64, and uploads over 5 MiB are rejected. Text previews are capped at 128 KiB (and never exceed 256 KiB when requested), terminal output at 64 KiB, and search results at 500 entries.

### Host device controls

Control Center reports real system-volume and display-brightness capabilities from the local host. These controls are disabled by default and remain unavailable in containers, on unsupported systems, with missing hardware, or with insufficient permissions. On a local Linux desktop, set `DEVICE_CONTROL_ENABLED=true` to opt in; mutations are accepted only from a loopback peer, volume uses the installed PipeWire/PulseAudio `pactl` interface, and brightness uses the kernel backlight interface under `/sys/class/backlight`. Notification chime volume is a separate application preference and never changes system volume.
