# Tauri + Vanilla

This template should help get you started developing with Tauri in vanilla HTML, CSS and Javascript.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


Template created! To get started run:
cd ets2-tool
cargo tauri android init

For Desktop development, run:
cargo tauri dev

## Garage Management

The Garage tab reads every garage from the active save and keeps the loaded
profile and save visible in the overview. Compact status cards, separate city
and garage-ID searches, ownership, size, headquarters and occupancy filters,
and persistent sorting keep the garage list navigable.

Garage details open in one keyboard-accessible modal with collapsible driver,
truck and trailer assignments. Purchase, size and headquarters changes use a
shared action-dialog layout with operation-specific loading text, inline error
details, automatic list refresh and a highlighted result card.

For ETS2 saves, the tool can purchase an existing unowned garage directly as a
five-slot large garage, change an owned garage between three and five slots,
purchase all currently unowned garages in one operation, and set an owned
garage as headquarters. Purchasing a small garage remains disabled until its
save structure is confirmed independently. Downsizing is allowed only when the
removed slots contain no truck or driver references. Each write checks the
loaded save hash, creates an automatic backup, preserves unknown fields and
existing assignments, replaces game.sii atomically, and verifies the resulting
save after reading it again. A bulk purchase creates one backup and performs
one atomic write; if every garage is already owned, the save is left unchanged.

Garage purchase and upgrade do not apply a financial transaction because no
unambiguous money/cost link was established in the supported save structure.
Garage purchase does not create trucks, drivers, or trailers. Optional truck
creation and AI-driver assignment remain unavailable because the project has no
verified creation workflow for those save units. Relinquishing ownership and
garage writes for ATS remain unsupported.

### Manual garage save verification

Never run this procedure against the only copy of a profile. Exit ETS2 first,
use the existing profile-clone workflow to create a separate ETS2 test profile,
and create a dedicated manual save in that profile. Keep an additional
filesystem copy of the complete test profile outside the ETS2 profile directory.
Do not enable ATS writes and do not commit the copied profile or its save files.

Before the first mutation, record the selected save name, the tool's reported
save hash, and the counts of `vehicle`, `driver_ai`, `driver_player`, and
`trailer` units in a decrypted inspection copy of `game.sii`. Select three
unowned garages A, B, and C, one empty three-slot garage, one empty five-slot
garage, and one five-slot garage with an assignment in slot 3 or 4.

| Test | Procedure | Required result |
| --- | --- | --- |
| Purchase 1 | Purchase unowned garage A | A has status 3 and five empty truck and driver slots |
| Purchase 2 | Purchase B immediately, without reopening the app, profile, save, or modal | A and B are owned |
| Purchase 3 | Purchase C immediately | A, B, and C are owned |
| Purchase all | Start from a copy where A, B, and C are unowned, then purchase all garages | Every unowned garage is owned, the HQ and existing assignments are unchanged, and one backup is reported |
| Purchase all no-op | Run purchase all again | No save change or backup is made and the tool reports that all garages are already owned |
| Upgrade | Change an owned three-slot garage from 3 to 5 | Status is 3 and slots 0 through 4 exist |
| Safe downgrade | Change an empty five-slot garage from 5 to 3 | Status is 2 and only slots 0 through 2 remain |
| Blocked downgrade | Try to downsize a garage with a truck or driver in slot 3 or 4 | The command is rejected and the save hash does not change |
| Headquarters | Change headquarters to another owned garage | Exactly one garage is HQ and the old HQ stays owned |
| Restart | Close and reopen the tool, then reload the copied save | All preceding changes remain visible |
| ETS2 load | Start ETS2 with the copied test profile | The profile and dedicated save load without an error |
| In-game check | Inspect A, B, C, their sizes, and headquarters | The game state matches the tool and inspected save |

Inspect these structures in the decrypted before/after copies of `game.sii`:

- The `economy` block must keep the same `garages` count, indices, order, and
  garage references.
- A purchase changes only the existing target `garage : garage.<city>` block
  from `status: 0`, `vehicles: 0`, and `drivers: 0` to `status: 3` with exactly
  `vehicles[0..4]: null` and `drivers[0..4]: null`.
- An upgrade changes `status: 2` to `status: 3`, changes both slot counts from
  3 to 5, preserves indices 0 through 2, and adds null entries 3 and 4.
- A safe downgrade changes `status: 3` to `status: 2`, changes both slot counts
  from 5 to 3, and removes only empty entries 3 and 4.
- The target garage's `trailers`, `productivity`, existing truck and driver
  references, and unknown fields must remain unchanged. Until the observed
  replacement `profit_log` block and its Unit-ID allocation are confirmed, the
  existing `profit_log` reference is reused only when it resolves uniquely to
  an existing `profit_log` block; otherwise the purchase is rejected.
- Headquarters changes only the active player's `hq_city` value. The old and
  new garage blocks, including ownership, capacity, trucks, drivers, and
  trailers, must otherwise remain unchanged.
- All unrelated garage blocks and all non-target units must remain unchanged.
  The total counts of `vehicle`, `driver_ai`, `driver_player`, and `trailer`
  units must match the before copy after every garage action.
- After every successful action, confirm that a new automatic backup exists and
  that the freshly reported save hash matches a new backend reload.

If post-write verification fails, stop the test. Confirm that the automatic
rollback restored the original hash and garage state before performing another
action.


## Trailer Change on the Road

The save editor includes a trailer switch workflow for the active save.

- Reads the active trailer from player assignment data.
- Lists owned trailers from the loaded save.
- Previews the selected trailer switch before writing.
- Supports an optional backup before applying the change.
- Re-reads the save and verifies the active trailer after writing.

For Android development, run:
cargo tauri android dev

## Troubleshooting

### Linux (KDE Plasma / Wayland)

If the app window is blank on Wayland, run with:

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo tauri dev
```

You can prefix the same environment variables to the app launch command outside of dev as well.

## Local Authentication (Career Mode)

This project includes a local/offline authentication system for the Career panel (SQLite + Argon2 password hashing).

### Test user (dev)

- Email: `admin@admin.de`
- Password: `admin123`
- Role: `admin`

The admin user is created/seeded automatically on first auth DB access and the password is stored only as a hash (never plaintext).

### Where login data is stored

**Database file**
- Path: `%LOCALAPPDATA%\\SimNexusHub\\logbook.sqlite` (Windows)
- Created automatically if missing.
- Fallback: if `dirs::data_local_dir()` is unavailable, the current working directory is used.

**Session file (remember-me token)**
- Path: `%LOCALAPPDATA%\\SimNexusHub\\auth_session.json`
- Contains only a persisted session token for “remember me” (no password, no email).

Source of truth for paths:
- `src-tauri/src/features/auth/db.rs`

### SQLite schema (auth-related)

Tables are created/updated automatically via “ensure tables/columns” (lightweight migrations):
- `src-tauri/src/features/auth/db.rs`

```sql
-- users
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL,
  email TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'user',
  company_id INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_login_at TEXT,
  consent_at TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  is_seed INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- sessions (remember-me)
CREATE TABLE IF NOT EXISTS sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  token TEXT,
  created_at TEXT NOT NULL,
  expires_at TEXT,
  last_used_at TEXT,
  revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);

-- recovery codes (hashed, one-time)
CREATE TABLE IF NOT EXISTS recovery_codes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  code_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  used_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_recovery_codes_user_id ON recovery_codes(user_id);

-- login events (privacy-friendly local MAU per installation)
CREATE TABLE IF NOT EXISTS login_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER,
  at_utc TEXT NOT NULL,
  year_month TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_login_events_year_month ON login_events(year_month);
CREATE INDEX IF NOT EXISTS idx_login_events_user_month ON login_events(user_id, year_month);
```

### What is stored / what is NOT stored (privacy)

Stored (minimum needed for local auth):
- Email + username (for login + display)
- Role (admin/user)
- Password hash (Argon2, salted) in `users.password_hash`
- Session token for “remember me” in `sessions.token` and `auth_session.json`
- Timestamps: `created_at`, `updated_at`, `last_login_at`, session timestamps

Not stored:
- No plaintext passwords
- No IP address, device fingerprint, geo location, or tracking identifiers
- No telemetry / online user tracking (local-only)

This is a technical, data-minimizing structure and **not legal advice**. For production, you likely want to add:
- Server-side auth (if you need global MAU), email delivery, and secure token flows
- Proper audit/event model, rate limiting, lockouts, and encrypted backups
- Optional database encryption at rest (depending on threat model)

### How login/logout works (technical)

Backend:
- Login/register: `src-tauri/src/features/auth/service.rs` (`login_local`, `register_local`)
- Password hashing: Argon2 in `src-tauri/src/features/auth/service.rs` (`hash_password`, `verify_password`)
- Session persistence: remember-me token written to `auth_session.json` and stored in `sessions`
- Logout: clears in-memory state + removes `auth_session.json` + revokes the session row (sets `revoked_at`)

Frontend:
- Header user menu + login/logout: `src/main.js`
- Career auth gate modal: `src/index.html` + `src/styles.css`
- State refresh: on startup `auth_restore_session` is called, then UI fetches `auth_get_current_user`

### Admin / Database view (local)

If you are logged in as `admin`, the header user menu shows **Admin / DB**.
- It displays: user id, email, role, created-at, last login, and whether an active session exists.
- It does **not** display any password or password hash.

## ETS2 Dispatcher Post-Write Validation

The ETS2 dispatcher write flow validates the written `game.sii` immediately after the file is updated.

- Validator source: `src-tauri/src/features/ets2save/post_write_validator.rs`
- Trigger point: `src-tauri/src/features/ets2save/injector.rs`
- UI output: Career Mode dispatcher detail panel, `Last Write Output`

The validator checks the full pointer chain:

1. `company.volatile.<company>.<city>` block exists
2. expected `job_offer[i]` pointer still exists in that company block
3. matching `job_offer_data : _nameless.*` block exists
4. written fields match the expected dispatcher payload:
   `cargo`, `target`, `shortest_distance_km`, `expiration_time`

Result interpretation:

- `post_write_valid = true`
  The write is structurally valid. If ETS2 still does not show the job, the remaining cause is ETS2 load/cache state. Load the exact quicksave that was written.
- `post_write_valid = false`
  The write is not valid for the expected depot/job chain. Use `validation.rootCause` and `validation.validationErrorCode` from the write output.

Root-cause mapping:

- `wrong_depot`: expected company block was not found after write
- `wrong_slot`: expected `job_offer` pointer is missing from the company block
- `write_corrupt`: `job_offer_data` block is missing for the selected pointer
- `cargo_mismatch`: written cargo token does not match the expected token
- `target_mismatch`: written target company does not match the expected target

The write output also includes an offer-slot scan so the selected `job_offer[i]` can be inspected when a depot contains multiple offer pointers.
