# ETS2 Save Edit Tool

## Project Coaches

The save editor provides separate Truck and Project Coaches careers in the
existing XP dialog, with muted blue truck and sage bus tabs. Each career retains
its own unsaved input and validation state when switching tabs. Saving updates
only the selected career and keeps the dialog open; closing discards remaining
drafts. Truck progression includes the existing level calculations, next-level
progress and access to the skill editor. Bus XP never use the truck level table
or alter truck skills; available bus statistics remain read-only in the dialog.
The Project Coaches tab reads bus statistics, referenced stops and discovery
counts, bus references, the active bus-job reference and bus trip logs.
Statistics other than XP and all references remain read-only. Distance and time
values are displayed without conversion because their units are not verified.

XP edits target the unique economy unit, use the existing automatic backup
mechanism and verify the saved content by reading it again. Unknown units,
fields and arrays remain in the save. Missing or invalid career XP fields are
not added; saves without Coaches fields continue to support truck XP.
The editor rejects saves changed since loading. No bus, route or job objects
are generated. Stop discovery remains read-only pending validated examples.

Checks: `cargo test --manifest-path src-tauri/Cargo.toml --bin ets2-tool`,
`node src-tauri/src/features/save_editor/frontend_tests.mjs`,
`node scripts/check_i18n.mjs` and `node scripts/check_ui.mjs`.
Browser checks require a local Chrome installation or `CHROME_PATH`.

## Application logs

Settings provides **User logs** with file metadata, level filters and a scrolling
viewer showing the newest entries first. The dialog loads automatically and
checks for changes while visible. **Export user logs** writes the complete
current log as a UTF-8 `.log` report with diagnostic header to Downloads,
independently of the selected filter. A native Save dialog is the fallback when
Downloads cannot be written. Filenames include local date/time and seconds;
collisions create a new name. Existing logs are never overwritten. **Clear
cache** only removes files in the dedicated temporary application cache.

Rust and JavaScript errors are recorded locally in
`<Tauri local application data>/support/logs/application.log.txt`. Entries append
across restarts, include operation identifiers and redact private paths and known
secret formats. The background writer rotates at 5 MiB; managed TXT archives
expire after 14 days and the total managed log size is limited to 20 MiB.
No reports are uploaded automatically. Existing legacy reports remain untouched.

Implementation and tests: [Logging](src-tauri/src/features/logging/README.md).

## ETS2 Data Location

The Settings tools include an ETS2 Data Location modal for switching between the default Documents folder and a custom `-homedir` root. Changing the location refreshes profile and save discovery and uses the selected folder for profiles, Steam profiles, local mods, config files and logs.

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
garage as headquarters. An empty owned garage can be relinquished when it is
not the headquarters and no other save unit references it. Purchasing a small
garage remains disabled until its save structure is confirmed independently.
Downsizing is allowed only when the removed slots contain no truck or driver
references. Each write checks the loaded save hash, creates an automatic backup,
preserves unknown fields and existing assignments, replaces game.sii atomically,
and verifies the resulting save after reading it again. A bulk purchase creates
one backup and performs one atomic write; if every garage is already owned, the
save is left unchanged.

Garage purchase, upgrade, and relinquishment do not apply a financial transaction because no
unambiguous money/cost link was established in the supported save structure.
Garage purchase does not create trucks, drivers, or trailers. The AI driver
manager can hire one or more existing recruitment candidates, optionally assign
available company trucks, change or remove truck assignments, and remove one or
more idle drivers. Drivers with an active job cannot be removed. Every driver
mutation uses the same backup, atomic-write, save-hash, reload, and rollback
safeguards as garage mutations. Garage and AI-driver writes for ATS remain
unsupported.

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
| Relinquish | Sell an empty owned garage that is not the HQ | The garage has status 0, zero vehicle/driver/trailer slots, productivity 0, and its profit log reference is retained |
| Blocked relinquish | Try to sell the HQ or a garage with a truck, driver, trailer, or external garage reference | The command is rejected and the save hash does not change |
| Upgrade | Change an owned three-slot garage from 3 to 5 | Status is 3 and slots 0 through 4 exist |
| Safe downgrade | Change an empty five-slot garage from 5 to 3 | Status is 2 and only slots 0 through 2 remain |
| Blocked downgrade | Try to downsize a garage with a truck or driver in slot 3 or 4 | The command is rejected and the save hash does not change |
| Headquarters | Change headquarters to another owned garage | Exactly one garage is HQ and the old HQ stays owned |
| Generate drivers | Generate multiple drivers for an owned garage, once without trucks and once with available trucks | Hired-driver arrays remain parallel, candidates leave the offer/pool lists, and each new driver occupies one garage slot |
| Truck assignment | Assign, change, and unassign a company truck for an idle driver | The truck reference moves between garage slots without duplication; unassignment is rejected if no empty storage slot exists |
| Remove drivers | Remove one idle driver and then multiple idle drivers | Driver references leave the hired and garage lists, return to the driver pool, and assigned trucks remain in their garage slots |
| Block active removal | Try to remove a driver with an active job | The command is rejected and the save hash does not change |
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
- After every driver action, confirm that `player.drivers`, `driver_flags`,
  `driver_readiness_timer`, and `driver_undrivable_truck_timers` have identical
  counts and contiguous indices. Hired drivers must occur in exactly one garage
  driver slot and in neither `drivers_offer` nor `driver_pool`.

If post-write verification fails, stop the test. Confirm that the automatic
rollback restored the original hash and garage state before performing another
action.
