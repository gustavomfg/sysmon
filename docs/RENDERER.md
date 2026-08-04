# Rendering Architecture

## Purpose

This document defines the rendering direction for SysMon.

SysMon must support a modern visual experience while remaining functional across terminals with different capabilities.

Rendering is treated as progressive enhancement.

---

## Rendering Pipeline

The intended flow is:

```text
System Snapshot
    ↓
Application State
    ↓
History and Animation State
    ↓
Visual Model
    ↓
Widget Rendering
    ↓
Terminal Backend
```

Collectors must never generate rendering primitives.

Rendering must never modify factual system data.

---

## Visual Model

A visual model contains presentation-ready values derived from application state.

Examples:

- animated CPU percentage;
- normalized Pulse intensity;
- active theme;
- selected widget;
- visible widget set;
- chart points;
- current animation phase;
- Timeline emphasis;
- reduced-motion state.

This layer prevents widgets from repeatedly interpreting raw metrics in inconsistent ways.

A formal visual-model type should be introduced only when the implementation demonstrates a clear need.

---

## Renderer Levels

### Level 1 — Standard Terminal

Required baseline.

Uses:

- ANSI colors;
- Unicode characters;
- Ratatui widgets;
- simple borders;
- text and meters.

The application must remain fully usable at this level.

### Level 2 — Unicode and Braille

Preferred default visual enhancement.

Uses:

- Braille chart markers;
- block elements;
- Unicode sparklines;
- denser terminal-native shapes;
- animated character fields.

This level should work in modern terminals without proprietary image protocols.

### Level 3 — Advanced Graphics

Optional future enhancement.

Potential technologies include:

- Kitty Graphics Protocol;
- Sixel;
- supported terminal-specific image protocols.

Advanced graphics must never become a mandatory dependency for core functionality.

---

## Rendering Backend Responsibilities

A backend may be responsible for:

- reporting terminal capabilities;
- drawing terminal-native content;
- supporting advanced visual surfaces;
- clearing previous visual resources;
- responding to resize;
- restoring terminal state;
- selecting appropriate rendering quality.

A backend must not be responsible for:

- collecting system metrics;
- choosing product behavior;
- interpreting command actions;
- defining application themes;
- mutating history.

---

## Capability Model

A future capability representation may include:

```text
True color support
256-color support
Unicode support
Braille support
Advanced graphics support
Image transparency support
Animation support
Cell dimensions
Pixel dimensions
```

Detection must be conservative.

When support is uncertain, prefer the standard renderer.

---

## Ratatui Role

Ratatui remains the primary foundation for:

- layout;
- widgets;
- text;
- borders;
- charts;
- input-driven redraw;
- test rendering.

Custom terminal effects should integrate with Ratatui rather than bypass the entire application architecture.

Direct terminal protocol use must be isolated behind a clear boundary.

---

## Frame Scheduling

Collection and rendering have different schedules.

Example direction:

```text
Metric collection: 1 Hz or configured interval
Animation update: up to 30 Hz
Rendering: while state changes or animation remains active
```

The renderer should not force expensive collection.

The loop should avoid busy waiting.

---

## Dirty Rendering

A future optimization may track whether another frame is necessary.

Possible dirty causes include:

- new system snapshot;
- active tween;
- Pulse animation;
- input event;
- focus change;
- Timeline event;
- overlay state;
- terminal resize;
- theme change.

Do not add a complex invalidation framework before profiling demonstrates a need.

---

## Terminal Resize

Resize behavior must be predictable.

On resize:

1. Recalculate layout.
2. Reevaluate rendering level.
3. Clamp selected and focused regions.
4. Simplify System Pulse if required.
5. Render a complete valid frame.
6. Avoid leaving stale advanced graphics.

---

## Rendering Quality

A future quality level may provide:

```text
Low
Balanced
High
Auto
```

Possible differences:

### Low

- reduced frame rate;
- simplified Pulse;
- fewer chart points;
- no decorative effects.

### Balanced

- smooth metrics;
- standard Pulse;
- normal chart density.

### High

- richer Pulse;
- denser charts;
- supported advanced effects.

### Auto

Select quality based on:

- terminal dimensions;
- renderer capability;
- recent frame performance;
- reduced-motion setting.

This is a future direction and should not be implemented without a documented requirement.

---

## Theme Integration

The renderer must use semantic theme tokens.

It must not introduce isolated hard-coded palettes.

System Pulse may use:

- `pulse_low`;
- `pulse_medium`;
- `pulse_high`;
- chart and accent tokens;
- semantic warning and danger colors.

Color transitions must preserve readability.

---

## Accessibility

Rendering must support:

- reduced motion;
- high contrast;
- meaningful symbols;
- monochrome fallback;
- visible focus;
- no rapid flashing;
- no dependence on color alone.

Animations must avoid abrupt luminance changes.

---

## Advanced Graphics Boundary

Advanced graphics should be isolated conceptually:

```text
rendering/
├── standard
├── braille
└── advanced
```

The exact module structure should follow demonstrated implementation needs.

Do not create empty abstractions only to match this document.

---

## Failure Behavior

If advanced rendering fails:

- clean up any allocated graphics resources;
- fall back to Unicode rendering;
- preserve the current application state;
- add a diagnostic Timeline event when appropriate;
- never leave the terminal corrupted.

---

## Testing

Tests should prioritize deterministic rendering logic.

Examples:

- capability fallback;
- layout selection;
- Pulse frame generation;
- Braille cell conversion;
- reduced-motion output;
- resize behavior;
- theme-token usage;
- unsupported-backend fallback.

Tests must not require a real Kitty terminal unless explicitly marked as integration tests.
