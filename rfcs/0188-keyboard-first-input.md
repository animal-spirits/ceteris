# 0188. Keyboard-first input: command registry, two keymaps, palette and query language

Status: proposed
Date: 2026-10-28
Design sections: 14.2, 14.8, 20 (Interface and Visualization), 20.2 (Console), 20.3 (Time controls); ADRs 0088, 0113, 0116, 0125, 0140, 0145, 0147, 0155, 0158, 0162
Phase: P3 (registry, standard keymap, palette, query language, hints), P3-P4 (vim mode, rule edits), P7 (war bindings)

## Context

No spec or ADR covers input. The native app (0088) has to decide how keys reach Slint, the map
and the core before panels are built, and the owner asked for keyboard-first use: a vim mode
(the owner uses vim), standard shortcuts that feel familiar, terminal-style interaction in the
manner of the Bloomberg Terminal, automation, and natural-language input. Brainstorm with a
playable demo

Two properties of the model shape the design. A branch is a parent plus an ordered list of timed
edits (14.1), which is the data model of an editor with history. Every causal change must enter
the edit log or the order list (14.2, 0145) so runs replay bit for bit.

## Options

1. Hard-coded shortcuts per panel, added as panels are built.
2. A single command registry with keymaps, a palette and an ex line on top of it.

## Decision

Option 2.

### Registry and invariants

- Every action is a named command with typed arguments. Menus, buttons, both keymaps, the
  palette and the ex line call the same commands. Where the CLI (0113) has the verb, the ex form
  takes the same arguments (`:bisect` and `ceteris bisect`), and the console of 20.2 logs ex
  commands, so a session exports as a scenario file and a recipe.
- Keys never write state. Commands that change the world append to the edit log or issue an
  order. Keymaps, hints, the palette, macros and watchers are presentation and enter no run hash.
- The cursor has three coordinates: place (cell, province, state, country), tick (monthly; daily
  while a war runs, 0145) and branch. Marks and the jump list store all three.
- An edit made from the keyboard lands on the view tick at commit. Entering any prompt pauses the
  clock by default (`autopause`), and the prompt shows the tick. Wall-clock time never sets a tick.

### Standard keymap (default)

Only conventions users already know, each binding taken from one source. The map cursor behaves
like Excel's active cell. Time uses debugger keys. Views and the palette follow VS Code, the
clock follows Paradox, and copying settings follows Factorio. On Mac, Cmd replaces Ctrl, and
platform idioms apply (Option+arrows, Cmd+[ and Cmd+]).

| Group | Bindings |
| --- | --- |
| Time | Space pause; 1-5 speed; +/- faster, slower; F5 run, Shift+F5 pause; F10 step one tick, Shift+F10 step back; Ctrl+F10 run to the timeline cursor; F9 watch the value at the cursor; Ctrl+G go to date; Ctrl+Home fork tick, Ctrl+End head; PgUp/PgDn events; F8 next alert |
| Map | Arrows move the cursor; Ctrl+arrows to the state border; Shift (+Ctrl) extends the selection; Shift+Space state, Ctrl+Space country, Ctrl+A all; WASD (physical keys) and mouse pan; Ctrl+= / - / 0 zoom; Home capital; Alt+Left/Right back and forward; Ctrl+1-9 save a group, Alt+1-9 recall; Shift+1-9 pinned map modes, M picker; tapping Alt toggles map numbers |
| Edits | F2 edit; Enter, Esc, Tab; Ctrl+Enter fill the selection; Ctrl+D fill from the anchor; F4 repeat; ] and [ step the active field; Delete removes the branch's edits on the selection; Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z; Ctrl+C / X / V, paste at the view tick; Ctrl+Alt+V paste special (ex ante or ex post, values or edits, re-target); Q pipette, Shift+right-click and Shift+click copy and paste settings |
| Find | Ctrl+Shift+P or F1 palette; Ctrl+P go to anything; Ctrl+F, F3, Shift+F3; Ctrl+H edit by query; Ctrl+Shift+F search all edit logs; F12 explain, Alt+F12 peek, Shift+F12 trade partners; ; map hints, Ctrl+; panel hints; Shift+F1 pedia |
| Branches and views | Ctrl+S write the scenario; Ctrl+Shift+S fork (save as); Ctrl+N, Ctrl+O; Ctrl+Tab recent branch; Ctrl+PgUp/PgDn cycle; Ctrl+W, Ctrl+Shift+T; Ctrl+\ split compare; Ctrl+B, Ctrl+J, Ctrl+` side panel, bottom panel, console; Ctrl+Shift+E/G/M/X edit log, branch manager, alerts, mods; F6 focus cycle; F11 or Alt+Enter full screen; Ctrl+, settings; Ctrl+K Ctrl+S keymap editor |

Where sources conflict, Paradox keeps the digits, Excel keeps the arrows, and VS Code keeps the
F-keys. Panels move off Paradox's F-keys to Ctrl+Shift+letter.

### Vim mode (opt-in)

- Mapping: branch is a buffer (`:ls`, `:b`, Ctrl+^ alternate); the edit list is the change list
  (`g;` `g,`); branch versions are the undo tree; aggregation grain is folding (`zo` `zc` `zR`
  `zM`, cells per 0115); compare is diff mode (`:diffsplit`, `]c`, `do`/`dp`); the explainer chain
  is the tag stack (Ctrl+], Ctrl+T); result lists (search, bisect probes, blame) are the quickfix
  list; `:help` is the pedia (0158); `ceterisrc` is the vimrc.
- Modes: normal, visual (`v` provinces, `V` states, Ctrl+V box), operator-pending, insert (value
  fields and notes), command-line, hint, and order-pending in war surfaces.
- Space motions: `hjkl` neighbours by bearing; `w`/`b` across the next state border in the last
  direction, `W`/`B` country; `gc` capital; `f` hints; `*`/`#` similar provinces; `n`/`N`.
  Time motions: `)`/`(` tick; `}`/`{` chronicle event; `]y` year, `]Y` decade; `]e` edit; `]a`
  alert; `]s` situation (0159); `G` head, `gg` fork tick, `{year}G`, `:YYYY-MM`. Speed is `+`,
  `-` and `{n}gs`, because digits are counts.
- Text objects: `i`/`a` with `p` province, `s` state, `c` country, `m` market, `u` union, `r`
  region, `/` search hits, `q` saved query, `e` edit target, `t` theatre, `F` front. `a` adds the
  bordering ring and a count widens it.
- Operators: `c` change, `>`/`<` and Ctrl+A/Ctrl+X step, `y` yank, `p` put ex post at the view
  tick and `P` put ex ante at the branch start, `d` delete edits, `x`, `r`/`R`, `~`, `J`
  (transfer, god mode), `!` ensemble over the target, `gb` blame, `gd`/`K` explain, `.` repeat.
  God-mode operators refuse unless god mode is on; in seat play the refusal names the in-world
  alternative.
- Registers are typed (values, edits, selection, keys, text). Putting edits into another
  branch is cherry-pick (0116). Uppercase marks cross branches; `'a` restores the place and
  `` `a `` restores place and tick.
- Leader is Ctrl+Space (owner, 2026-10-28). Space stays pause in both keymaps. The standard
  keymap's Ctrl+Space is not active in vim mode. On macOS Ctrl+Space switches input source by
  default; the first launch in vim mode says so and offers to rebind.
- Ex commands follow the registry; date and mark ranges are time windows (`:2008,2012 blame`),
  `'<,'>` is the visual selection. War: `o{verb}{target}` issues an order (0147) and `]u` jumps to
  the next formation without orders.

### Palette, terminal mnemonics and hints

- The palette fuzzy-matches commands (showing their key in the active keymap and their ex form),
  places in English and local names (0143), dates, branches, marks, saved queries and goods.
  Titles follow the seat's regime register (0140), and canonical names always match too.
- Terminal grammar, after the Bloomberg Terminal: subject, optional type, mnemonic, Enter (`Spain
  WEI`, `Sevilla DES`). Mnemonics include DES, GP, HP, ECO, TOP, WEI, MOST, FA, BOP, TRD, RV, HR,
  EQS, ALRT, BTST, BLAM, BSCT, OOB, LOGI and HELP. Lists print numbered rows that open by number.
  Panels carry link groups that change subject together. Monitors are saved tables of subjects
  with windowed columns. A "Terminal" workspace arranges four linked panels.
- Hint labels (Vimium): two home-row letters on every province (`f`), every panel control (`F`,
  Ctrl+;), every route (`t`) and every map entity (`gf`), assigned in reading order so labels are
  stable inside macros.

### Query language

One language for search, selection, `:where`, `:g`, peer sets (0155) and saved queries: set
operations, flags (`coastal`, `port`, `rationed(steel)`), comparisons with checked units,
`key=name`, `near(place, k)`, `rank`, `pct(column, peers)`, `routes(through, edge)`, and time
qualifiers (`@date`, `over a..b`). It is deterministic (no wall-clock, no randomness), and its
text form is what an edit by query stores.

### Automation

- Macros, `.` and recipes (`.cet` files of ex commands) are presentation. Only the edits they
  produce enter the log.
- `:where {query} {edit}` stores the predicate and re-evaluates it at the edit's tick on replay
  (0155). `:g/{query}/{edit}` stores the set matched now; `:g/{query}/normal {keys}` runs keys at
  each match.
- Rule edit, a new edit type in 14.2: `:when {query} {edit}` with a firing policy (once per
  target by default, each tick while true, cooldown, until a date). It is evaluated at a fixed
  point of the tick pipeline on the previous tick's state, and each firing is logged as an
  ordinary edit with its own tick, which blame and bisect see. In seat play a rule uses only the
  seat's levers, as a standing policy next to the commitments of 0162. God-mode rules carry the god
  flag.
- Sweeps (`:sweep`, BTST) create sibling branches that form an experiment (14.1), and run
  headless as `ceteris sweep`.
- Watchers (`:pauseon`, `:forkon`, `:alert`, F9) act on the UI only.

### Natural-language input

- A controlled-English grammar ships with the app and works with AI off. It covers edits,
  rules, selections, navigation and "why" questions, and compiles them into ex text. Anything it
  cannot read goes to the intervention compiler of 0125, if the user has turned it on.
- Compile, preview (ex text, matched set on the map, tick, in-world or god), then commit. The log
  stores the compiled text, with the sentence kept as a comment, plus the model id and a prompt
  hash for model output. Replays never call a model.
- The app asks for ambiguous places, goods or dates and never guesses. A sentence outside the edit
  types of 14.2 is refused, and the nearest edit types are listed. Questions route to the
  explainer and blame. With the model on, the 0125 research assistant answers, citing run hashes.

### Implementation

- One input router in Rust ahead of Slint: winit `KeyEvent` goes to a keymap engine (mode
  machine, pending-key trie with `timeoutlen` 700 ms, counts, registers) that emits registry
  commands. Slint receives keys only while a Slint text field has focus. Esc always reaches the
  engine.
- Bindings match `logical_key`. Camera panning matches `physical_key`. IME and dead keys pass
  through in text modes. Tab and Ctrl+I are distinct under winit.
- A which-key panel appears 400 ms after any prefix. Keymaps are TOML files and `ceterisrc` holds
  ex commands, both in the config folder and never in a pack or run. Mods may ship keymaps.
- AccessKit names every hint target, so the keyboard path is also the screen-reader path.

## Migration

- P3: registry, standard keymap, palette, query language, hints, edits by query, console as an ex
  line. P3-P4: vim mode once the edit log and branch manager exist; rule edits; controlled-English
  grammar. 0125 compiler as that ADR schedules it. P7: order grammar and war bindings with
  surfaces W2-W5 (0166).
- DESIGN 20.2: the console logs ex commands that export as a scenario file and a recipe, which
  replaces "Python-equivalent commands" (already superseded by 0113). DESIGN 14.2 gains the rule
  edit type. DESIGN 20.3: time controls take the bindings above. Both the living document and the
  `specs/DESIGN.md` snapshot are updated in the same change.

## Tests

- Every registry command is reachable from the palette. No two bindings collide within a context
  of either keymap.
- Replaying a recorded key log on a fixture gives the same edit log and state hash on every
  platform. Changing keymap or entering hint mode changes no state hash.
- A rule edit replays to the same firing ticks. Removing it in a fork removes exactly its firings.
- An edit by query replays to the same cell set on another machine (0155).
- Every sentence in the controlled-English test corpus compiles to its expected ex text with AI
  off. The stored edit of a model-compiled sentence replays with the model disabled.

## Consequences

The workbench can be driven entirely from the keyboard. Newcomers find familiar keys, vim users
get a grammar that maps onto branches and edits, and analysts get a terminal grammar. The cost is
about 6-10 part-time weeks across P3-P4 for the router, registry, palette, query language and
both keymaps. The rule edit type adds one mechanism to 14.2 that blame, bisect and Shapley
attribution must handle.
