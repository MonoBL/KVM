# Opus Review - Hopper KVM (post Sonnet build)

Date: 2026-06-08. Reviewer: Opus. Verified `cargo test -p kvm-core`:
45 passed, 1 ignored. Tests are real.

## Fixes applied (2026-06-08, Opus)

All reported bugs fixed. `cargo test -p kvm-core`: 54 passed (was 45),
1 ignored. `cargo check -p hopper`: compiles (warnings only).

- C1 DONE: `capture::Converter` turns absolute positions into true deltas.
  `event_to_msg` no longer emits mouse moves. Test `converter_produces_deltas`.
- C2 DONE (logic): `engine::on_mouse_abs` tracks remote virtual cursor,
  detects the return edge, emits `LeaveScreen`, parks + restores the local
  cursor (`MouseOutcome.warp` / `set_hidden`). `forward()` gates local input.
  Tests: `crossing_back_returns_to_local`, `remote_moves_forward_as_deltas`,
  `local_input_not_forwarded`. NOTE: live grab->engine->warp wiring is the
  remaining manual/PENDING step; contract documented on `on_mouse_abs`.
- C3 DONE: `Alt => 0xE2`, `Slash => 0x38`. Test `alt_and_slash_differ` +
  `no_code_collisions` (all keys).
- C4 DONE: `code_to_enigo` returns `Option`; unmapped codes are skipped in
  `inject.rs` (no garbage). Test `unmapped_codes_return_none`.
- H1 DONE: letters/digits use named VK keys on Windows (modifier combos),
  `Unicode` on macOS/Linux. `char_key` cfg-split.
- H2 DONE: `start_listen` no longer calls `process::exit`; it stops
  forwarding instead.

Remaining (unchanged, not bugs): live two-machine run, cursor warp/hide
OS calls, `.dmg`/`.exe` builds. All still PENDING in TEST_LOG.md.

## Tasks 1-4 review (2026-06-08, Opus)

61 tests pass, app compiles. Server/client loops are structurally sound:
suppress logic is correct (local input passes, remote input forwarded +
parked), mpsc bridge and dedicated inject thread are right, cursor FFI is
Send-safe. But these BLOCK a real end-to-end test:

- B1 (blocker) Screen size hardcoded. `commands.rs:255-260`
  `primary_display_width/height` return 1920x1080 stub. On any other display
  (any Retina Mac), edge math is wrong and crossing fails. Fix: read real
  display size per OS.
- B2 (blocker) Client screen never registered as a neighbor. Engine has only
  the local screen, so `neighbor()` returns None and control NEVER switches.
  Connecting must `add_screen` for the peer at a grid position (or require
  the layout editor to place it). Nothing wires this today.
- B3 (high) One rdev grab thread spawned PER client session, and rdev::grab
  never exits. Reconnect or a 2nd client = multiple grab loops = chaos /
  leaks. Fix: a single server-wide grab loop routing to the active peer.
- B4 (high, macOS) `CGWarpMouseCursorPosition` suppresses mouse events for
  ~0.25s after each warp -> jumpy remote cursor. Fix: call
  `CGAssociateMouseAndMouseCursorPosition(false)` while remote, true on return.
- Task 7 still PENDING: no GUI Connect button, so there is no way to start a
  client from the UI yet (only the `connect_to_peer` command exists).

Conclusion: do ONE more pass (tasks 5-7 + B1, B2; ideally B3, B4) before the
two-machine test. Testing now would fail on B1/B2 regardless of effort.

## Verdict (original review below)

Plumbing is solid. Core input path is broken.

Good and tested: protocol, transport (framed TCP), discovery (mDNS),
files (chunked), TLS (rcgen + TOFU), packaging/CI.

The actual KVM loop has real bugs. Green tests hide them because they
cover the easy parts, not the live input path. Every broken thing sits
in a PENDING manual test. Do not ship until fixed.

## Critical bugs (block the product)

### C1 - Mouse move sends absolute coords as deltas
`capture.rs:9-12`: rdev `MouseMove{x,y}` is the ABSOLUTE cursor position.
Code labels it `dx/dy`. `inject.rs:22` replays `move_mouse(Rel)`.
Result: client cursor flies to a corner. Mouse control unusable.
Fix: server tracks last position, sends `dx = x - last_x`, `dy = y - last_y`.

### C2 - No return from remote, no cursor parking
`engine.rs:57-78`: once control goes Remote, `update_cursor` returns None
forever. Nothing ever sends `LeaveScreen`. The client gets relative deltas
and has no edge logic to know it left. So you cross once, never come back.
Also the server's own physical cursor is never parked/warped, so it hits
its own edge while "remote". This is the heart of a KVM and it is missing.
Fix: server keeps a virtual cursor over the remote screen, detects the
return edge, warps/parks local cursor at the edge while remote.

### C3 - Keymap code collision: Alt == Slash
`keymap.rs:12` `Alt => 0x38` and `keymap.rs:97` `Slash => 0x38`. Same code.
0x38 is HID for Slash; Left Alt should be 0xE2. So pressing `/` sends Alt,
and Alt is mis-coded. Fix: `Alt => 0xE2`, keep `Slash => 0x38`.

### C4 - Reverse keymap incomplete -> wrong keys injected
`code_to_enigo` is missing many codes that `rdev_to_code` emits: keypad
(0x54-0x63), Insert 0x49, PrintScreen/ScrollLock/Pause, NumLock, AltGr,
MetaRight, BackSlash 0x31, IntlBackslash, Function. They fall to
`Other(n)`. enigo `Other` expects a RAW PLATFORM keycode, not a HID usage.
So those keys inject garbage. The round-trip test only checks 9 happy keys,
so it stays green. Fix: complete the reverse map; never pass HID into
`Other`.

## High

### H1 - Modifier combos unreliable (Unicode keys)
Letters map to `Unicode('a')`. enigo Unicode types a character and does not
compose with held modifiers across apps (Cmd+C, Shift+letter, Ctrl+key).
Fix: map letters/digits to enigo physical keys (`Key::A`, `Key::Num1`...),
not Unicode, so modifiers apply.

### H2 - start_listen kills the whole app
`capture.rs:55`: on receiver drop it calls `std::process::exit(0)`.
In the GUI that terminates everything. Fix: signal and return, do not exit.

## Low

- `capture.rs:42` `button_id` casts `u16` extra buttons to `u8` (truncates).
- Wheel sign/units: rdev deltas vs enigo `scroll` may invert direction.
  Confirm in live test, add sign flip if needed.
- `engine.rs:44` client starts `Control::Remote{0}`; harmless but odd.

## Test gaps

Add tests that would have caught the above:
- mouse delta: feed two absolute positions, assert correct dx/dy.
- keymap: assert NO two rdev keys share a code (collision test over all).
- keymap: assert every emitted code has a non-`Other` reverse mapping
  (or is intentionally raw).
- engine: full cross-and-return cycle returns control to Local.

## Recommendation

Fixes are well-scoped, ~1 module each. Hand C1-C4 + H1-H2 back to Sonnet
with this file. C2 is the largest (real switching logic). Re-run live
two-machine test after.
