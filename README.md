# crownos-files

The CrownOS file manager. The library is the engine behind the window: the
directory model the UI renders, and the workers that keep it in sync with the
disk, run file operations, and talk to the rest of the system.

## Architecture

| Module | What it does |
|---|---|
| `fs` | Streamed directory listing, sorting, inotify watching, mount table, folder sizes |
| `model` | Compact entries, selection, history, special folders, the ops the UI applies |
| `jobs` | Copy, move, delete and trash on a prioritised worker pool, with conflicts and undo |
| `mime`, `thumbs`, `open` | Type detection, freedesktop thumbnails, default apps and launching |
| `preview` | The quick-look client for `crownos-preview` |
| `devices::udisks` | UDisks2 drives: live volume list, automount, mount, unlock, eject |
| `devices::storage` | Free space (`statvfs`) and the per-folder breakdown of the storage card |
| `devices::connect` | Paired phones and tablets from `crownconnect`, and sending files to them |
| `privileged` | Running operations on other users' files as root, through pkexec |
| `tags` | Tags in the `user.xdg.tags` attribute, and `tag:` search filters |

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
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Path dependencies are expected next to this checkout: `../crownos-ipc`,
`../crownos-preview`, `../crownconnect-linux`.

Features, all on by default:

- `thumbs-pdf`: PDF thumbnails through poppler;
- `thumbs-video`: video thumbnails through GStreamer;
- `connect`: "Send to device" through crownconnect.

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
