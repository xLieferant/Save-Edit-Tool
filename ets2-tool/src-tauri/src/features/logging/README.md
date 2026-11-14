# Local application logging

The existing shared logger routes Rust `log` records, legacy user logs, verified
save-operation events and frontend failures to one bounded background writer.
There are three persisted levels: INFO, WARN and ERROR. Ordinary DEBUG/TRACE data
is excluded; existing trace errors are promoted to ERROR. Records contain a local
timestamp with milliseconds and offset, module/action, session/operation IDs,
message, technical cause and available safe context. No raw IPC arguments or save
contents are collected. Existing IPC results and rejections are preserved.

## Files and limits

`support/logs/application.log.txt` is below Tauri's local application data path.
The file is appended across restarts and rotated before exceeding 5 MiB. Managed
archives end in `.txt`, expire after 14 days and share a 20 MiB cap with the active
log. A panic tries to flush the writer, then writes synchronously; if the writer
is busy or panicked, a separate emergency TXT archive preserves the panic.
An abruptly terminated process or unavailable disk can still lose unwritten data.
Queue overload is bounded and counted rather than blocking normal application work.
Old JSONL/PDF/ZIP reports are neither created nor automatically migrated/deleted.

Settings opens **User logs** with four metadata cards, last-modified time,
INFO/WARN/ERROR filters and a separate scrolling viewer. Entries are read from
the current persisted log, including earlier sessions, sorted newest first and
kept together with their continuation lines. Filters use the loaded snapshot;
scrolling appends further rows from memory. A visible open dialog checks for
changes every five seconds. Unchanged revisions do not reload or rebuild the
viewer, and closing stops the checks. No manual refresh or log-clear action is
shown in this dialog.

**Export user logs** writes the complete current file independently of the
selected filter or rendered rows. UTF-8 reports use the local export time in
`SaveEditTool-UserLogs_YYYY-MM-DD_HH-mm-ss.log` and contain an export header,
the installed application version, OS/architecture and entry count. The default
destination is Downloads; a native Save dialog is used if it cannot be written.
Existing names receive a numeric suffix through exclusive file creation.
Original logs and application data cannot be overwritten. Canceling the native
dialog does not report success. The existing legacy TXT export adapters remain
available. No email or network transfer is performed.

## Privacy and cache boundaries

Text is redacted before entering the writer or in-memory history: private paths,
emails, named passwords/tokens, bearer credentials and common JWTs. Raw save
content is discarded. Redaction is conservative and cannot recognize every
possible unlabelled secret; inspect the exported report before sharing it manually.
Reports apply these masks again without the producer's 8 KiB text limit and also
mask Basic credentials and private-key blocks. This preserves the full retained
file rather than truncating exports to the viewer or in-memory history.
Cache cleanup only visits `.tmp`/`.cache` files in `support/cache`, excludes known
save/profile/backup/settings directories, and requires a matching inventory token and
explicit user confirmation. Traversal, symlinks and Windows reparse points are
rejected, including ancestors. Profiles, savegames, backups and settings outside
that dedicated temporary directory are never cleanup candidates.

## Verification

From `ets2-tool`:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib shared::logs::diagnostic
cargo test --manifest-path src-tauri/Cargo.toml --bin ets2-tool features::logging
node --test src-tauri/src/features/logging/frontend_tests.mjs
node src-tauri/src/features/logging/ui_smoke.mjs
cargo tauri dev --no-watch
```

`ui_smoke.mjs` uses local Chrome and synthetic commands by default, checking
English/German, filters, full exports, write/load errors, cancellation, long
paths and lines, and dark/light layouts at 480–1600 px. `CHROME_PATH` overrides
the Chrome executable. With `--native`, it checks the real Tauri WebView on CDP
port 19223, records tagged validation events and restores the original language.
Adding `--export` to `--native` also writes one report to Downloads through the
real export button and verifies its filename, installed version, all log levels
and masking with the ERROR filter selected.
Neither mode opens or modifies game saves. Rust tests use isolated temporary
directories for filenames, collisions, fallback, redaction and source protection.
