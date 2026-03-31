# Simulator

This crate contains the generic simulator core used by the embedded app simulators.

## Editor Model

- The editor uses a reducer shape: `(state, action) -> (new_state, effects)`.
- The main persisted UI state is `AppState<T>`.
- Interactive commands are represented by `Command`.
- External work is represented by `Effect<T>`.
- The current effects are:
  - `SaveTrace { trace }`
  - `Quit`

Initial trace loading is not an effect today. It happens during session initialization before the TUI loop starts.

## Replay Recording

Live TUI sessions record the initial state plus every dispatched `Command`.

- Press `W` in the live simulator TUI to save a replay file.
- Replay files are written next to the trace file.
- Filenames use a UTC timestamp, for example:
  - `2026-03-31_11-26-46.replay.json`

Replay files are separate from trace files.

High-level replay format:

```json
{
  "kind": "simulator-replay",
  "version": 1,
  "initial_state": { "...": "full AppState" },
  "commands": ["..."]
}
```

The replay stores the full initial `AppState`, not just the trace, so UI-only state such as cursor position, dialog state, scroll offset, and terminal size can be reproduced exactly.

`Quit` is not recorded because it is an effect, not a command.

## Replay Mode

Replay mode reconstructs state on demand from:

- `initial_state`
- the prefix of recorded `commands`

State index semantics:

- `0` = initial state before any command
- `1` = state after the first command
- `N` = state after the first `N` commands

The replay TUI:

- opens at state `0`
- uses `Left` / `Right` to move between states
- keeps state-driven UI content intact
- shows replay metadata in title chrome instead of overwriting the status panel

## CLI Examples

The current app-specific binary lives in `embedded/info-panel-lib`.

Open a normal trace:

```bash
cargo run --features simulator-ui --bin info-panel-simulator -- simulator1.json
```

Open a replay file in replay mode:

```bash
cargo run --features simulator-ui --bin info-panel-simulator -- 2026-03-31_11-26-46.replay.json
```

This also works explicitly:

```bash
cargo run --features simulator-ui --bin info-panel-simulator -- --replay 2026-03-31_11-26-46.replay.json
```

Print all recorded commands:

```bash
cargo run --features simulator-ui --bin info-panel-simulator -- --show-actions 2026-03-31_11-26-46.replay.json
```

Render the screen after a specific action and exit:

```bash
cargo run --features simulator-ui --bin info-panel-simulator -- --render-action 1 2026-03-31_11-26-46.replay.json
```

`--render-action` errors if the requested index is out of range.

## Rendered Snapshots

`--render-action` renders through the same ratatui UI code and emits ANSI escapes to stdout.

That means it preserves:

- cursor highlighting
- selection background
- invalid-row coloring
- dialog highlighting

This output is faithful to the final cell buffer, but popup overlays can look crowded in scrollback because the result is a static terminal snapshot rather than a live alternate-screen session.

## Notes

- Replay exactness depends on the same simulator/runtime/rendering code.
- Resize commands are recorded, so terminal-size-dependent layout can be reproduced.
- The replay system is intended for debugging and UI reproduction, not as a stable long-term interchange format.
