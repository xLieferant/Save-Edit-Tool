# ETS2 Save Edit Tool

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
