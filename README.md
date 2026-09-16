# FluidaOS

FluidaOS is a modular browser desktop with persistent local data, movable windows, and integrated productivity apps. Its original glass-and-gradient interface is built with TypeScript, CSS design tokens, and no proprietary assets.

## Run locally

Requires Node.js 20 or newer.

```bash
npm install
npm run dev
```

Open `http://localhost:3000`. For a production-style run use `npm start`; run validation and service tests with `npm test`. User content is stored under `os_storage/`, while configuration and app records live in `os_storage/.fluida/`.

## Architecture

- `src/client/core` contains bootstrap, shared state, and the API client.
- `src/client/shell` implements window lifecycle, focus, dragging, resize, and task switching.
- `src/client/apps` contains the built-in application views and safe terminal commands.
- `src/client/styles` contains design tokens, responsive layout, themes, wallpapers, and motion rules.
- `src/server/routes`, `services`, and `middleware` separate HTTP transport, persistent repositories, storage access, validation, and typed errors.
- `src/shared/types` defines domain contracts used by client and server.

## Built-in features

The desktop includes draggable shortcuts, a running-app dock, resizable and maximizable windows, task switching, global app search, notifications, Quick Settings, Files, Editor, Notes with auto-save and folders, a restricted Terminal, System Monitor, Calculator history, Settings, four safe wallpapers, theme switching, and reduced-motion support.

The terminal intentionally does not execute host commands or spawn processes. Its 20 commands are implemented in-process through the contained file service:

| Command | Behavior |
| --- | --- |
| `help`, `pwd`, `date`, `clear` | Show commands, the storage-relative directory, server time, or clear output |
| `cd`, `ls`, `tree`, `find` | Navigate, list, recursively display, or search contained folders |
| `cat`, `head`, `tail`, `stat` | Read text or show safe file metadata |
| `mkdir`, `rmdir`, `touch` | Create folders, remove empty folders, or create files |
| `write`, `append` | Replace or append quoted text |
| `cp`, `mv`, `rm` | Copy, move, or remove files (`rm` never recursively removes folders) |

Arguments support single or double quotes. Absolute paths and traversal outside `os_storage/` are rejected; output, argument counts, line counts, and execution time are bounded.

Files provides metadata used for folder navigation, breadcrumbs, previews, sorting, grid/list layouts, selection, and safe copy, move, rename, and recursive-delete workflows. Text previews should remain size-bounded; unsupported content is represented as metadata rather than executed.

Settings is organized around Appearance, Accessibility, Desktop, Notifications, Storage, Applications, and System preferences. Nested preferences are persisted without discarding sibling values, and can be reset to defaults. The shell honors the browser's reduced-motion preference in addition to the saved animation level.

Five additional built-ins are registered throughout the shell: **Clock** (world clocks, alarms, timers, stopwatch), **Calendar** (events and agenda), **Photos** (storage-backed image metadata), **Archive Manager** (non-executing archive records), and **App Center** (application status and manifest metadata).

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
| `POST /api/fs/read`, `/write`, `/mkdir`, `/rename` | Perform validated file operations |
| `POST /api/fs/copy` | Safely copy a contained file or folder |
| `DELETE /api/fs` | Delete a contained file or directory |
| `GET /api/apps` | List built-in apps and pinned state |
| `GET`, `PUT /api/settings` | Read or update appearance and behavior |
| `GET`, `POST`, `DELETE /api/notes/:id` | Search, save, and remove notes |
| `GET`, `PATCH`, `DELETE /api/notifications/:id` | Manage notification state |
| `GET /api/system` | Report storage, runtime, and app totals |

Storage paths must be relative. Absolute paths, null bytes, traversal outside `os_storage`, invalid request bodies, and unknown settings are rejected.
