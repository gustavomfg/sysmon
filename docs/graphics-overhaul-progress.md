# Graphics overhaul — progress

## Recovery and scope

- Started 2026-10-12, baseline `334fb1f` on `main`.
- Work branch: `graphics/telemetry-instruments`; no push or publication.
- Preexisting work consists of deletions of `AGENTS.md` and 18 old documents.
  These deletions are preserved. A binary patch and status were saved in
  `/tmp/sysmon-graphics-20261012/` before any edits.
- SYSTEM PULSE's field, dynamics, palette and telemetry integration are preserved.

## Reconnaissance (confirmed from source and execution)

- Rust 2024 / Rust 1.95; ratatui 0.30.2, crossterm 0.29, sysinfo 0.39.5.
- Native terminal UI, no browser/canvas. Three existing theme palettes.
- `SystemInfoCollector` → `LiveSnapshot` → `LiveMetricsHistory` → metric models
  → semantic render blocks → `TerminalRenderer` → ratatui cell buffer.
- Collection uses monotonic `Instant`; refresh interval is 1, 2 or 4 seconds.
  Animation/rendering runs separately. Models are rebuilt on collection.
- SYSTEM PULSE is fed through health and pulse behavior engines; its renderer
  is independent of the lateral metric cards.
- Eight bounded time series (120 samples), but insertion shifts the whole array.
- Sparkline presentation loses timestamps, caps data at 60 values, and changes
  the displayed sample count with width. Resize consequently changes history.
- CPU/GPU percent graphs autoscale silently. Storage has capacity meters and
  current I/O readouts but its card does not draw the available read/write histories.
- Existing `TestBackend` tests and an external cell/PNG capture helper are usable
  for deterministic visual comparisons. Main UI never substitutes demo data.
- Baseline: `cargo test --offline --all-targets`: **309 passed**, no failures.
- Local btop 1.4.7, Arch package 1.4.7-1.1, Apache-2.0. Tagged official source
  downloaded into the investigation directory, executable left untouched.

## Priorities

1. High: preserve timestamps in presentation; make time windows independent of
   width; stop fabricating a full-width history from one sample.
2. High: explicit fixed percent domain, labeled rate scales, real extrema.
3. High: separate storage capacity from I/O history; preserve device identity.
4. Medium: RAM/swap and VRAM capacity instruments alongside activity traces.
5. Medium: expanded history/detail, readable thermal indicators with trusted limits.
6. Medium: bounded graphics cost, measured baseline/final renderer comparison.
7. Low: restrained grid, endpoint accents, theme consistency and layout spacing.

## Current direction

Reuse the semantic model and Braille rasterizer. Introduce a small shared
instrument frame for temporal axes, scales, peaks and availability; retain
real values and gaps. Use fixed 0–100% for utilization, capacity bars for RAM,
VRAM and storage, and paired series with a shared rate domain for network/I/O.
No new runtime dependency is planned. Source research and implementation
decisions will be recorded in the accompanying graphics documents.

## Execution log

- Reconnaissance and baseline tests complete.
- Implemented bounded circular histories, relative monotonic timestamps,
  duration windows, rate hysteresis, fixed percent domains, labeled extrema,
  capacity rails, paired expanded flows and core details.
- Corrected binary units and CPU comparison direction/interval. Added trusted
  critical-temperature metadata and neutral rendering for unknown limits.
- Expanded history initially exposed stack overflow from by-value getters;
  borrowing semantic model getters resolved it. No test stack limit was raised.
- Added immutable shared semantic models and a pixel cache for lateral cards;
  invalidation covers telemetry, focus, visibility, theme and resize. Pulse's
  animation clock and implementation are unchanged.
- Added terminal output compatibility (16/256 colors, monochrome, ASCII).
- Temporary artifacts did not survive a session interruption. Subsequent
  investigation artifacts are kept in `target/graphics-overhaul/`, and the
  baseline can be reproduced from Git without touching the working tree.
- Added collection-quality flags for startup/new interfaces/counter resets;
  invalid network samples produce gaps. Storage/GPU/network identity changes
  start fresh histories rather than joining different devices.
- Final compact flows separate both directions even at one/two rows. GPU
  capacity retains fractional binary totals (e.g. 7.5 GiB rather than 7 GiB).
- Final checks: **331 tests passed**, format, strict all-target Clippy and
  release binary/example build passed. Baseline had 309 passing tests.
- Rendered 12 fixture cases: five terminal sizes, three themes and five
  expanded metrics. Pulse cells were identical to baseline in all five sizes.
- Five paired frozen-frame benchmark rounds: main view 20–34% less draw time;
  CPU/RAM expansions cost 10–11% more. Three animated rounds show much smaller
  total gains because the existing animated Pulse path dominates the cost.
- Four bounded live runs exited successfully. Total CPU remains high on this
  host in both versions; this is documented without claiming low overall usage.
- Real PTY smoke passed focus, expansion, search, theme, resize and terminal
  restoration in Unicode/True Color and ASCII/mono. The harness responds to
  Crossterm cursor-position requests; its first incomplete attempt was discarded.
- Research, architecture, reproducible captures, raw benchmark data and known
  limitations are recorded in `graphics-research.md`, `graphics-architecture.md`
  and `graphics-overhaul-results.md`. Runtime dependencies/license unchanged.
- Implementation and validation complete. Local commits separate telemetry
  instruments (`7c50684`), terminal compatibility (`2303094`), capture/smoke tools
  and evidence/documentation. No push/publication performed or authorized.
- Remaining opportunities are explicitly scoped in the results document:
  per-device histories, richer memory pressure telemetry, emulator diversity
  and profiling the existing animated Pulse cost. No required implementation
  step remains pending for the delivered lateral instruments.

## Continuation — 2026-10-22

- Resumed at clean commit `7596235`; the user had authorized and committed the
  19 earlier deletions separately. No new user modifications were present.
- Baseline remains 331 passing tests. Investigation artifacts and baseline
  binaries are isolated in `target/pulse-performance-20261022/`.
- Instrumented only a research copy of the Pulse pipeline. An eight-second
  160×45 run generated 148 scenes, including 68 repeated 100 ms ticks. The
  animated input comparison bypassed the intended raster cadence while the
  health/behavior model eased between telemetry samples.
- Implemented a small integration correction: animated scenes consume the
  latest input on the next tick; reduced-motion scenes still respond directly
  to telemetry. Theme, dimensions and motion-mode changes invalidate immediately.
  No field, particle, noise, lighting, color or raster algorithm was changed.
- Validation complete: 333 tests, strict all-target Clippy, format and release
  build passed. Both terminal smoke modes passed with full restoration.
- Instrumented follow-up: 80 scenes in eight seconds, zero duplicate ticks.
  Twelve uninstrumented paired runs (three per version/size) measured median
  one-core CPU of 29.09% → 16.74% at 80×24 and 27.39% → 20.39% at 160×45.
  Individual runs and instrumented-stage variability are preserved in evidence.
- All 12 full first-frame captures are byte-identical. Regression tests compare
  refreshed scenes to fresh rendering at the same phase/input. Intermediate
  frames are deliberately reused until the next 100 ms tick and available draw.
- Results, raw measurements and the research-only instrumentation patch are
  documented in `graphics-overhaul-results.md` and `docs/graphics/`.
  No runtime dependency or Pulse engine change; local commits only, no push.
