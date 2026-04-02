# Simulator Editor Specification

## Goal

Build a generic, project-independent simulator editor core in `embedded/common/simulator/src/editor` that can:

- load a saved simulator run from JSON
- replay it against a project-provided simulator adapter
- show the full derived event timeline
- allow editing the scripted items that define the run
- keep working even when the current script becomes invalid after code changes

The editor owns document structure, replay, selection, mutation, and derived view state. Project crates provide the concrete operation/result types and the adapter traits needed to replay and render them.

## Scope

The editor core is not the runtime wrapper itself. It sits above:

- `runtime/` for executing runs
- `analysis/` for replay helpers such as `possible_next_events(...)`

The editor core should be usable from a terminal UI, but the terminal layer must stay thin. Most behavior should live in pure state transitions and pure derived-view computation so it can be tested without a terminal.

## File Format

Saved runs are JSON files discovered from the current directory. Filenames are the user-visible titles.

Each file should contain a marker and version:

```json
{
  "kind": "simulator-run",
  "version": 1,
  "items": []
}
```

`items` is a single ordered list. Each item is one of:

- an `outbound` item that names a derived outbound request event
- an `inbound` item that injects an inbound event into the run

Each item must use separate fields for item side and protocol event kind:

- `type`: `outbound` or `inbound`
- `event_type`: `create_sync`, `create_async`, `return_sync`, `resolve_async`, `abort_async`, `cancel_async`, or `drop_result`

The file does not store runtime numeric ids. Instead:

- request events define a symbolic `id`
- events that act on an existing request or result ref use symbolic `target`

The symbolic id space is generic and includes both:

- request ids introduced by `create_sync`, `create_async`, and inbound `create_async`
- result refs introduced inside `result` payloads as `{ "ref": "..." }`

This keeps saved runs stable across replay even if runtime ids change.

Example:

```json
{
  "kind": "simulator-run",
  "version": 1,
  "items": [
    {
      "type": "outbound",
      "event_type": "create_sync",
      "id": "boot_reason"
    },
    {
      "type": "inbound",
      "event_type": "return_sync",
      "target": "boot_reason",
      "result": { "type": "boot_reason", "value": "software" }
    }
  ]
}
```

## Script Model

The document is a single ordered item list.

### Outbound Items

An `outbound` item does not create an event. It matches one already-derived outbound request event at that point in replay and gives it a symbolic id.

Supported outbound request kinds:

- `create_sync`
- `create_async`

An outbound item should contain:

- the outbound event kind
- symbolic `id`
- optional symbolic `target` when the request acts on a previously returned continuation ref

Any `drop_result` item should contain:

- `event_type: drop_result`
- symbolic `target` referring to an earlier continuation ref

### Inbound Items

An `inbound` item injects a real inbound event into the replay.

Supported inbound kinds:

- `return_sync`
- `resolve_async`
- `abort_async`
- `cancel_async`
- `create_async`
- `drop_result`

Rules:

- `create_async` defines a symbolic `id`
- `return_sync`, `resolve_async`, `abort_async`, and `cancel_async` use `target`
- completion-event `target` must refer to an earlier symbolic request id
- any request item may also use `target` to refer to an earlier continuation ref that the request operates on
- continuation refs are introduced inside `result` payloads with `{ "ref": "..." }`

This supports both directions of async flow:

- outbound-created async ops completed by inbound events
- inbound-created async ops completed by outbound events

It also supports the full continuation matrix:

- sync or async completion may introduce zero, one, or many continuation refs inside its `result`
- later sync or async requests may target any previously introduced continuation ref
- either side may later drop a continuation ref with `drop_result`

## Continuation Refs

Some result payloads return continuation-style values such as streams, readers, subscriptions, cursors, or other reusable handles. Saved JSON must represent those values explicitly with symbolic refs embedded at the point where the value appears:

```json
{
  "type": "inbound",
  "event_type": "resolve_async",
  "target": "http_get_1",
  "result": {
    "type": "http_response",
    "body": { "ref": "body_1" }
  }
}
```

If one completion returns multiple continuation values, each value should carry its own ref:

```json
{
  "type": "inbound",
  "event_type": "return_sync",
  "target": "open_pair",
  "result": {
    "type": "pair",
    "left": { "ref": "left_stream" },
    "right": { "ref": "right_stream" }
  }
}
```

Later requests may target those refs regardless of sync or async direction:

```json
{ "type": "outbound", "event_type": "create_sync", "id": "read_1", "target": "body_1" }
{ "type": "outbound", "event_type": "create_async", "id": "watch_1", "target": "left_stream" }
```

Top-level `id` remains the identity of the request event itself. Continuation refs live inside `result` payloads rather than at top level because one completion may introduce multiple reusable values.

## Result Lifecycle

Continuation refs remain valid until they are explicitly dropped.

Use a `drop_result` item when whichever side currently holds a continuation value drops it and will no longer retain or use it:

```json
{ "type": "outbound", "event_type": "drop_result", "target": "body_1" }
```

The same event kind may also appear as an inbound item when the wrapper/environment drops a continuation value:

```json
{ "type": "inbound", "event_type": "drop_result", "target": "body_1" }
```

After a ref is dropped, later requests targeting that ref are invalid.

Replay validation should enforce:

- a continuation ref must be introduced before it is targeted or dropped
- continuation ref ids must be unique across the whole document
- `drop_result` targeting an unknown or already-dropped ref is invalid
- requests targeting a dropped ref are invalid
- once either side drops a ref, it is closed globally for later replay

## Replay Model

The source of truth is the saved item list. The visible event list is derived.

Replay should:

1. start a fresh run through the project adapter
2. collect the initial outbound events
3. walk the saved item list in order
4. for each outbound request item, match it against the currently available derived outbound request
5. for each `drop_result` item, validate and apply the result-drop to replay state
6. for each other `inbound` item, build the concrete inbound runtime event and push it into the wrapper
7. extract any continuation refs introduced by the inbound result payload and add them to replay state
8. collect the newly emitted outbound events
9. continue until the list ends or replay becomes invalid

The editor must never treat regenerated outbound events as editable source data.

## Visible Timeline

The UI should navigate a full visible timeline, not just the saved item list.

The visible timeline includes:

- initial outbound events emitted at start
- each saved `inbound` item as a script row
- each saved inbound or outbound `drop_result` item as a script row
- derived outbound events emitted after each inbound event

Saved outbound request items are persisted in the JSON document, but they should normally annotate their matched derived outbound events rather than appear as separate visible rows in the main timeline. Inbound and outbound `drop_result` items are real script actions and should appear as visible rows.

Main timeline labels should stay concise:

- do not show runtime numeric ids such as `CreateAsync#7`
- do not show symbolic ids such as `sleep_20ms`
- keep symbolic request/response linkage in derived metadata for future timeline and pairing visuals

The cursor lives in visible-timeline space, but navigation and visual selection operate on whole steps rather than individual rows. A step is the initial start block, one saved inbound item together with the derived outbound rows that follow from it, or one saved inbound or outbound `drop_result` item.

This allows the UI to:

- move through all shown steps
- highlight pending requests
- highlight the paired event for the current row
- insert items at the visible boundary the user is looking at

The main trace view should include a left timeline gutter. Requests should use directional glyphs so starts and completions are visually distinct: a request starts with a start marker, remains visible with vertical continuation while pending, and ends with a completion marker. Concurrent operations should occupy separate compact lanes so overlapping work is easy to see.

## Validity Model

The editor must support invalid traces.

Replay should compute:

- the last valid saved item position
- the first replay error, if any
- the derived visible timeline up to the invalid point

Opening a trace should still succeed when replay becomes invalid after the initial prefix. Only unreadable or structurally invalid JSON should prevent the trace from opening.

Rows after the valid prefix should be rendered as invalid. The user must still be able to:

- move through invalid rows
- edit, insert, delete, or reorder items after the last valid point
- repair an earlier item so that later items become valid again on the next replay

This is required to support workflow where code changes reorder requests and the saved run needs only small fixes.

## Pairing And Pending State

The derived view should compute request/completion relationships.

At minimum it should identify:

- `CreateSync` <-> `ReturnSync`
- `CreateAsync` <-> `ResolveAsync`
- `CreateAsync` <-> `AbortAsync`
- `CreateAsync` <-> `CancelAsync`

The derived view should also track which requests are still pending at the current replay point so the UI can highlight them.

The derived view should also track continuation refs and their lifecycle so the UI can show:

- which refs are currently open
- which request or completion introduced each ref
- whether a ref has been dropped
- which later requests target a given ref

## Editing Model

The editor should behave like a state machine over a persistent document.

Core editing operations include:

- move cursor up/down/start/end by step
- move by page and half-page
- center the current step in the viewport
- enter and exit visual selection mode
- insert a new item below the cursor
- edit the current item
- delete the current item or selected range

The editor may also surface adapter-defined trivial continuation chains at the end of the valid trace prefix. These should appear as dim preview rows and be acceptable with a single command that appends the full unambiguous trivial-success chain.

In the current implementation, trivial continuation previews appear inline in the event list as grey ghost rows at the end of the valid trace. They are not selectable, and `.` accepts the full trivial chain.

In the current incremental implementation, edit and delete operate on the current visible step. Saved outbound marker items may be removed or replaced together with their paired inbound row when they form the symbolic binding for the edited event.

Selection is vim-like:

- enter visual mode
- keep an anchor at the start position
- extend selection by moving the cursor step-wise
- apply operations such as delete to the selected step range

## Insert And Edit Flow

When inserting below the cursor, the editor should:

1. compute the replay position associated with that visible boundary
2. compute the valid next event possibilities at that point
3. present the allowed item kinds
4. let the project adapter provide the concrete form UI for operation/result payloads
5. build the saved item and insert it into the document
6. rerun replay and refresh derived state

Editing an item follows the same adapter-driven form model.

## Internal State

The editor core should keep three categories of state.

### Persistent Document

- file path
- parsed run document

### Ephemeral UI State

- visible-timeline cursor
- optional visual selection anchor
- mode
- transient insert/edit dialog state

### Derived Replay State

- regenerated visible timeline
- mapping between visible rows and saved items
- symbolic id resolution
- continuation ref resolution and lifecycle
- pairing relationships
- pending requests
- valid-prefix boundary
- replay error
- possible insertions for the active boundary

Derived replay state must be recomputed from the document, not edited directly.

## Architecture

The editor implementation should be split into pure core logic and a thin terminal frontend.

Recommended structure:

- document model
- replay engine
- editor state and commands
- derived view-model builder
- terminal frontend

The main update loop should be reducer-like:

- input command
- previous editor state
- next editor state
- optional side effects such as autosave

The renderer should consume a prepared view model and avoid recomputing domain logic itself.

## Adapter Boundary

Project crates should provide traits for:

- constructing a fresh simulator run
- translating saved items into concrete runtime events
- matching outbound items against derived outbound runtime events
- formatting operations and results for display
- defining insert/edit forms for app-specific payloads
- serializing and deserializing app-specific operation/result payloads

The generic simulator crate should own the editor state machine, replay pipeline, and generic terminal frontend.

Insert and edit forms are adapter-owned. The generic frontend should provide the modal lifecycle and a popup area, while the project adapter may render directly with `ratatui` inside that area and return the exact JSON items that should be written back to the run file.

Saved sync and async result payloads may represent either successful completion or failure. The generic simulator protocol does not need separate error event kinds; project adapters may encode success and error within their app-specific result JSON.

`Now`/current-time reads should be modeled as normal sync request/response events. When the adapter offers a default value for a `Now` response, it should derive that value from elapsed trace time rather than hidden runtime state.

## Persistence

The editor should autosave after each document mutation.

The trace picker should:

- scan the current directory for `.json` files
- load files that identify themselves as simulator runs
- use filenames as titles
- allow creating a new file
- allow copying an existing file to a new filename

## Testability

Most editor behavior should be testable without a terminal.

Priority test areas:

- replay of valid and invalid scripts
- symbolic id, target, and continuation-ref resolution
- pending and pairing derivation
- cursor movement and visual selection
- delete, insert, edit, and move operations
- repairing an invalid run into a valid run
- autosave effects emitted after mutations

Terminal-specific tests should be minimal smoke tests. The bulk of coverage should target pure state transitions and pure derived view computation.

## Initial Implementation Order

1. Define the saved document and item model.
2. Define the adapter traits for replay, matching, formatting, and forms.
3. Implement the pure replay pipeline from saved items to visible timeline.
4. Implement editor state, commands, and selection behavior.
5. Implement derived pairing, pending, validity, and insertion data.
6. Add unit tests around reducer behavior and replay.
7. Add the terminal frontend on top of the prepared view model.
