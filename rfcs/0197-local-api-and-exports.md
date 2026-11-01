# 0197. Local API, exports and deep links

Status: proposed
Date: 2026-10-28
Design sections: 2.1, 14.8, 20.2, 20.3, 21.1; amends ADR 0113 (scripting interface); see ADRs 0125, 0128, 0134, 0135, 0188, 0196
Phase: P3 (read-only `ceteris serve`, exports, deep links), 1.x (writes, lockstep, Python client, app as server)

## Context

Users want to reach the simulator from their own programs: Python and R notebooks, Excel,
dashboards, and custom code. ADR 0113 made the P3 experiment API the `ceteris` CLI driven by a
scenario file, with outputs as Arrow IPC and Parquet, and left a thin PyO3 client open "if
notebook-driven runs prove necessary". That covers batch work after the fact, but not reading a
live or paused run, streaming values while it runs, or steering it from a script.

ADR 0188 already routes every action through one command registry whose verbs match the CLI.
The run store and snapshots (14.3) are Arrow. The LLM research assistant of ADR 0125 and the
coupling protocol of ADR 0135 both need a programmatic way in.

Any outside access must keep 2.5 (7): the same pack, parameters, seed and intervention list give
bit-identical output. Assessment: https://claude.ai/artifact/3rJLuwN8cDUXDmA2vbwijC

## Options

1. Files only (ADR 0113 as is).
2. A PyO3 module linking the core into Python.
3. A local server: the core behind a localhost socket, commands from the registry, tables as
   Arrow; clients in any language.

## Decision

Option 3 (owner, 2026-10-28: read-only first; drop the PyO3 client).

### The rule

- Reads are free. A read names a branch and a tick and is served from the live head or a
  snapshot. It never changes state.
- Writes are edits. An outside program writes only by registry commands that append an edit of
  14.2 or an order, with an explicit tick, tagged with its source (`api` and a client name) and
  the god flag where it applies. A tick at or before the head of a live branch forks, as in the UI.
- One queue. Commands from every window (0196) and every client enter one queue, applied at the
  tick boundary where 0188 evaluates rule edits, in (tick, source, sequence) order. Wall-clock
  arrival never orders commands.
- Replays never call a client. The log holds what landed and when.

### `ceteris serve`

- The headless core kept alive behind a socket bound to 127.0.0.1 only. A token file in the
  config folder authenticates clients, with separate read and write scopes. No LAN or remote
  binding (DESIGN 2.4).
- Commands are JSON-RPC; their names and arguments are the registry's, so `:bisect`, `ceteris
  bisect` and the `bisect` call take the same arguments. Tables return as Arrow IPC streams, or CSV
  and JSON on request. A subscription streams chosen columns at each tick.
- `ceteris api schema` prints the commands and table columns as JSON Schema. The API version
  follows the registry and the run-store schema; a break gets a `ceteris migrate` path (0136).
- 1.0 ships the read scope only: open a run or branch, read at a tick, subscribe, and the read-only
  registry commands (explain, blame, query). Writes, forks and lockstep follow in a 1.x minor.

### Writes and lockstep (1.x)

- A program that answers tick N while the clock runs can land at different ticks depending on
  speed. The run still replays, but the script does not reproduce the experiment. In lockstep
  mode the core waits at each tick boundary until every registered controller has answered or
  passed. Headless runs default to lockstep; the app defaults to free-running and records.
- An external controller in seat play uses only the seat's levers, as rule edits do (0188).
- Lockstep is the transport for the coupling protocol of 0135.

### Clients

- Python: a pure-Python package over the socket using `pyarrow`, replacing the PyO3 client of
  0113 and DESIGN 19.1. Nothing Python enters the cargo workspace, the build or CI. It ships after
  `serve`, in 1.x.
- Other languages use the socket directly (R `arrow`, Julia, curl); no bindings are maintained.
- The desktop app can open the same endpoint, off by default (1.x).
- An MCP adapter over `serve` is how the 0125 tools reach a run; 0125's validator still applies.

### Exports and deep links (1.0)

- Ctrl+C on any table copies TSV. Any panel exports to xlsx (`rust_xlsxwriter`, MIT or
  Apache-2.0), any map mode to GeoPackage, and maps and charts to SVG and PNG.
- Excel reads Parquet or CSV through Power Query, including a `serve` URL that returns CSV.
- `ceteris://` links carry a run hash (0128), branch, tick and place, and open the app at that
  cursor.

### Not decided here

- Mechanism plugins stay sandboxed wasm in the extended profile (0134); the API is not a way
  around that. In-app panel mods wait on the Slint licence question of 0088.
- Exposing our own API is not exposing Slint's APIs, so 0088's licence question does not apply.

## Consequences

Analysts get live, scriptable access from any language with one schema, and the ADR 0125 tools
and 0135 coupling get their transport. Serving data only on localhost keeps sources each user
downloaded (such as EPR, CC BY-NC-SA) on that user's machine. Costs are estimates: read-only
`serve` about 3 part-time weeks in P3, exports and deep links 1-2, writes and lockstep 1-2 more
in 1.x, the Python client about 1. The registry becomes a public interface, so command renames
need deprecation. All of it is a minor under 0136.

## Migration

- DESIGN 2.1: the batch row reads "CLI, scenario files or the local API" instead of "Python API".
- DESIGN 14.8: scripting goes through the CLI, scenario files and `ceteris serve`.
- DESIGN 19.1 and 19.3: no PyO3 bindings and no `ceteris-py` crate (as 0113 already removes).
- DESIGN 20.2: the console's commands are the API's commands. 20.3: ensemble streaming uses
  subscriptions.
- DESIGN 21.1, P3: "Python experiment API" becomes "CLI and read-only local API".
- ADR 0113, scripting interface: the PyO3 client option is withdrawn.
