# 0196. Multiple windows and workspaces

Status: proposed
Date: 2026-10-28
Design sections: 20.2; see ADRs 0088, 0116, 0188, 0197
Phase: P3 (window-agnostic panel host, workspace files), P3-P4 (tear-off panels)

## Context

The lab's core tasks are side by side: baseline next to fork, the map next to the explainer, a
fan chart next to the edit log. On one screen these compete for space; most users of the tools
the standard keymap draws on (Excel, VS Code, Bloomberg Terminal) spread them across monitors.
Paradox games keep one window.

ADR 0088 makes the app one native process: winit window, Slint panels, the map drawn by our wgpu
pipeline into a texture from Slint's rendering notifier. ADR 0188 already has link groups (panels
that change subject together), a cursor of place x tick x branch, and a "Terminal" workspace of
four linked panels. Nothing says whether a panel may live outside the main window.

A panel host that assumes one window is cheap to build and expensive to retrofit once P3 has
dozens of panels. Assessment

## Options

1. One window. Split compare (Ctrl+\\) and docking only.
2. Panels can be torn off into windows; the map stays in the main window.
3. Option 2, plus further map windows, each with its own map mode and cursor.

## Decision

Option 2 (owner, 2026-10-28). Option 3 would need Slint to drive the map texture in two windows
at once, which is unverified; the owner judged pop-out panels enough.

- Panel host. A panel names no window. It reads core columns through Slint models and submits
  registry commands (0188); the window that holds it is presentation. This is a P3 rule for every
  panel.
- A window is a workspace: docked panels plus one link group (0188). Its cursor either follows
  the main window's cursor or is pinned to a branch, a tick, or both. A window pinned to the
  baseline at the main cursor's tick shows the baseline's panels beside the fork's.
- Tear-off: drag a panel's tab out, or `Ctrl+K O` (VS Code, "open in new window"; Cmd on Mac).
  `F6` focus cycling crosses windows. `Ctrl+N` keeps its meaning from 0188. The registry gains
  `window.new`, `window.close`, `window.pin` and `window.follow`; all are presentation.
- The map stays in the main window. Small map-like panels (an inset, a cartogram, a sparkline map)
  are drawn as ordinary panel images, not as a second instance of the map pipeline.
- Workspace files. Windows, their monitor and position, panels and link groups are saved in the
  config folder, like keymaps. They enter no run hash and are never part of a pack or a run. A
  window whose monitor is missing at load opens on the primary monitor.
- Determinism. All windows share the single in-process core. Windows only read and submit
  commands, which enter the one command queue of 0197; a command's window is not part of the edit.
- The Tauri app (`app/`) gets no window work; it is replaced by the native app.

## Consequences

Analysts can put the explainer, the edit log, fan charts and the branch tree on other monitors.
The panel-host rule costs little in P3 if adopted with the first panels; tear-off is about one
part-time week after that (estimate). The native spike of ADR 0088 is unchanged. A second map
window can be revisited in a later minor; it would need its own spike. This is a UI change, so it
ships as a minor under ADR 0136 if it misses 1.0.

## Migration

- DESIGN 20.2: panels may live in any window; workspaces are presentation.
