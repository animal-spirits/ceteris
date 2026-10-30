# 0245. Control labels, irregular shares and how advances relabel cells

Status: accepted (owner, 2026-10-30); amended by 0246 (rear security)
Date: 2026-10-30
Design sections: 11.5, 11.6; amends ADRs 0115, 0149 and 0154
Phase: P7 (2.0-beta), stage W2

## Context

ADR 0115 gives each H3 resolution 6 cell one controller label. ADR 0154 gives each cell shares of
control held by the state, insurgents and criminal groups, so that selective violence can follow
the contested-but-leaning gradient (Kalyvas 2006). The two do not agree. ADR 0149 gives each front
segment an advance rate in km per day but no rule for which cells change controller, or who
controls cells with no formation near them, such as a rear area left undefended (Kyiv 2022, ISIS in
2014). Explainer: https://claude.ai/artifact/DKERrMw26DBAB5jQ18aaGn

## Options

Control per cell:

1. A single controller label for interstate fronts, plus a sparse share field only in cells where
   irregular war runs.
2. Shares by actor in every cell; fronts become steep share gradients.
3. A single label everywhere, insurgent and criminal control included.

Advance to relabel:

1. Cells swept by a segment's advance change controller; cells with no formation change when only
   one side has formations within a radius.
2. Control spreads each day by flood fill from formations until it meets enemy control.
3. Cells change controller only under formations.

## Decision

Option 1 in both (owner, 2026-10-30).

- **Controller label.** Every cell keeps one controller label (0115). Interstate fronts, occupation
  and the economy's control shares per province read it.
- **Irregular shares.** A cell where irregular war is active (0154, 0069) also holds shares for
  the state, insurgent groups and criminal groups, summing to one. The field is sparse: a cell
  enters it when an armed group first operates there and leaves when the state share stays above
  a threshold for a set time (register parameters). Violence, taxation and recruitment inside
  those cells read the shares. The controller label of such a cell is the actor with the largest
  share.
- **Swept cells.** Each hour a segment advances (0149, 0209), cells whose centres fall between the
  old and new front line along its frontage take the attacker's label.
- **Zone of control.** A cell holding no formation takes a side's label when that side has a
  formation within the zone-of-control radius and the other side does not. Contested gaps keep
  their current label. The radius depends on mobility class and terrain, is a register parameter,
  and is evaluated once per day after the hourly phase, in cell-ID order.
- **Encirclement** (0145 flood fill) is unchanged; pockets keep their label until they surrender
  or are reduced.
- Owner labels are untouched by all of this; sovereignty still changes only on political events
  (0115).

## Consequences

- Fronts stay crisp for transport cuts (0115) and the map; the Kalyvas gradient exists where
  irregular war needs it, at storage cost only in those cells.
- Undefended areas fall to whoever moves in, at a pace tied to the 0149 advance and formation
  mobility, without the long jumps a flood fill would produce.
- New register parameters: share-field entry and exit thresholds, and the zone-of-control radius
  by mobility class and terrain.
- Tests in W2: a constant-input segment relabels exactly the swept cells; a cell with formations
  of both sides within the radius never changes label; the share field sums to one in every cell
  and is empty in a run with no irregular war.
