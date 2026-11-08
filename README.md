# crownos-files

The CrownOS file manager, as a cargo workspace:

| Crate | What it holds |
|---|---|
| `crates/app` (`crownos-files`) | The window |
| `crates/core` (`crownos-files-core`) | The engine: directory model, listing, watching, file jobs, MIME, thumbnails, opening |
| `crates/devices` (`crownos-files-devices`) | What lies outside the filesystem: drives, free space, privileged operations, tags, paired devices |
| `crates/helper` (`crownos-files-helper`) | The root helper pkexec runs for privileged operations |

## Architecture

| Module | What it does |
|---|---|
| `core::fs` | Streamed directory listing, sorting, inotify watching, mount table, folder sizes |
| `core::model` | Compact entries, selection, history, special folders, the ops the UI applies |
| `core::jobs` | Copy, move, delete and trash on a prioritised worker pool, with conflicts and undo |
| `core::{mime, thumbs, open}` | Type detection, freedesktop thumbnails, default apps and launching |
| `core::preview` | The quick-look client for `crownos-preview` |
| `devices::udisks` | UDisks2 drives: live volume list, automount, mount, unlock, eject |
| `devices::storage` | Free space (`statvfs`) and the per-folder breakdown of the storage card |
| `devices::connect` | Paired phones and tablets from `crownlink`, and sending files to them |
| `devices::privileged` | Requests, validation, the helper's executor and the pkexec client |
| `devices::tags` | Tags in the `user.xdg.tags` attribute, and `tag:` search filters |

Blocking work runs on blocking threads and reports over channels; D-Bus and
IPC clients run on tokio.

### Privileged operations

When a job fails with `EACCES`/`EPERM` (`JobError::needs_privilege`), the UI can
retry it through `PrivilegedClient`, which runs
`pkexec /usr/lib/crownos-files/crownos-files-helper`. The request goes to the
helper's stdin as a length-prefixed postcard frame; progress and the result
come back the same way on stdout. Closing stdin cancels. The helper:

- refuses to run as root without `PKEXEC_UID`;
- resolves every path and refuses `..`, relative paths, `/` and the folders
  right under it, and anything in `/proc`, `/sys`, `/dev` or `/boot/efi`, also
  through symlinks;
- runs the same copy, move and delete code as the app, and gives what it
  creates in the caller's own folders to the caller.

pkexec exit code 126 (dialog dismissed) is reported as `Cancelled`, 127 as
`NotAuthorized`.

## Build

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Path dependencies are expected next to this checkout: `../crownui`, `../crownos-ipc`,
`../crownos-preview`, `../crownlink-linux`.

Features, all on by default:

- `crownos-files-core/thumbs-pdf`: PDF thumbnails through poppler;
- `crownos-files-core/thumbs-video`: video thumbnails through GStreamer;
- `crownos-files-devices/connect`: "Send to device" through crownlink.

## Install paths

| File | Path |
|---|---|
| `crownos-files` | `/usr/bin/crownos-files` |
| `crownos-files-helper` | `/usr/lib/crownos-files/crownos-files-helper` |
| `data/crownos-files.desktop` | `/usr/share/applications/crownos-files.desktop` |
| `data/org.crownos.files.helper.policy` | `/usr/share/polkit-1/actions/org.crownos.files.helper.policy` |

Make Files the folder handler with
`xdg-mime default crownos-files.desktop inode/directory`. Drives need
`udisks2`; polkit prompts need a running polkit agent.

`packaging/PKGBUILD` installs all of the above; a `v*` tag builds it and
attaches the package to the GitHub release.
