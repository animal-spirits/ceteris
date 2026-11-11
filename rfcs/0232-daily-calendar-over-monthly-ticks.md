# 0232. Daily calendar over monthly ticks

Status: proposed
Date: 2026-10-28
Design sections: 4.3, 4.4, 4.5, 19.2, 20.3; ADRs 0016, 0145, 0188, 0208, 0209

## Context

The owner asked whether the economy could tick weekly or daily so time feels faster, as in
Paradox games. Assessment

A tick costs the same work whatever its length, so weekly ticks cost about 4.3 times and daily
ticks 30 times per simulated year: 0.1 to 0.25 years per second weekly, 30 to 60 s per year
daily, against 0.5 to 1 year per second monthly (19.2). Calibration falls from about 3,000 runs
overnight to about 700 or 100. Price and inventory adjustment speeds, transit lags and
cross-tick trade clearing (0016) are per month, and most inputs are monthly, quarterly or annual,
so a shorter tick means re-estimation and adds no data.

ADR 0208 already counts the date in days between monthly ticks and interpolates the map. What
gives the illusion away is that every economic and political event happens on the 1st.

## Options

1. Daily or weekly ticks for the whole economy.
2. Weekly trade or weekly province markets inside the monthly tick.
3. Stagger monthly work across the month by province.
4. Keep monthly ticks; present a daily calendar with dated events (amends 0208).

## Decision

Option 4.

- **Ticks.** Economy and politics stay monthly (4.3); war stays daily with the sparse hourly
  phase (0145, 0209).
- **Event days.** Every discrete event of a monthly system (war declaration, election, default,
  coup, law passed, crisis) carries a day of the month. Events already scheduled by sampled
  next-event times (19.4) take the day of their sampled time. Others draw it from their own
  stream, keyed as in 4.5 with a reserved draw index, so no other stream moves. The day orders
  presentation and the event log; the event's state effect still applies at its tick.
- **Presentation.** The date counts daily (0208). An event is presented, and watchers, alerts and
  pause-on rules (0188) fire, when the presented date reaches its day, never earlier.
- **Labelling.** Values between ticks stay interpolated on the map only. Panels, tooltips and
  exports show tick values and name the tick (0208); the event log shows the day and the tick it
  took effect.
- **Edits.** An edit made mid-month lands at the next tick boundary after the view tick (0188).
- **Weekly trade rejected.** Trade is the largest line in 19.2 and already uses its 30 ms at 10
  goods; weekly trade would cost about 4.3 times that. Shipments take weeks to months
  (transit pipeline), so weekly clearing would only speed price convergence, which the ablation
  test (15.7) would have to show is worth it. Weekly province markets alone (about 6 ms per
  month) are left open for P3 if price responses within a month prove needed.
- **Staggering rejected.** Spreading the month's systems over days gives identical results and
  no smoothness gain, since the simulation already runs off the frame loop (0208). Updating
  provinces on different days changes the model, and trade normalizes all destinations at once.

## Tests

- Event days change no state hash: a run with day stamps equals one without at every snapshot.
- Day draws are stable under adding an entity or a system (4.5).
- A seat-play replay presents no event before its day.

## Consequences

Time reads daily at the cost of one draw per discrete event. The tick, calibration budget and
goldens are unchanged. A player who reads the log closely sees that monthly events take effect
at the tick, not on their day; the log says so.

## Migration

- P3: day stamps on events and the reserved draw index (core RNG).
- App shell (0208): presentation by day.
- DESIGN 20.3: one sentence on event days when accepted.
