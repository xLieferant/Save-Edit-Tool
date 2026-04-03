# Tauri + Vanilla

This template should help get you started developing with Tauri in vanilla HTML, CSS and Javascript.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


Template created! To get started run:
cd ets2-tool
cargo tauri android init

For Desktop development, run:
cargo tauri dev

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
