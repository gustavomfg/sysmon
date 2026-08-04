# Architecture

## Purpose

This document defines the target architecture for the next generation of SysMon.

The current functional implementation must evolve incrementally. Existing metric collection, rendering behavior and tests should be preserved while responsibilities are separated into clearer modules.

The architecture should support:

- Independent metric collection
- Historical metric storage
- Smooth animations
- Modular widgets
- Configurable layouts
- Multiple themes
- Different terminal rendering capabilities
- Predictable performance
- Testable components

---

## Architectural Flow

```text
Collectors
    ↓
System Snapshot
    ↓
Application State
    ↓
History and Insights
    ↓
Animation State
    ↓
Layout and Widgets
    ↓
Renderer
    ↓
Terminal Backend
```

Each layer must have a clear responsibility.

Rendering code must not collect system metrics.

Collectors must not know how their data will be presented.

Widgets must consume prepared state rather than execute operating-system commands directly.

---

## Collectors

Collectors retrieve raw information from the operating system and external system utilities.

Examples:

- CPU
- Memory
- Swap
- GPU
- Network
- Storage
- Processes
- Temperatures

Collectors are responsible for:

- Reading system metrics
- Validating collected values
- Applying collection intervals
- Handling unavailable resources gracefully
- Returning structured data

Collectors are not responsible for:

- Rendering
- Colors
- Layout
- Animations
- User interaction

Expensive collectors may use independent refresh intervals and caching.

---

## System Snapshot

A system snapshot represents the latest successfully collected metrics.

It should contain factual system data without presentation-specific state.

Examples:

```text
cpu_usage
memory_usage
network_rates
disk_usage
processes
gpu_metrics
collected_at
```

A snapshot should be immutable after creation whenever practical.

---

## Application State

Application state represents the current interactive state of SysMon.

It may include:

- Latest system snapshot
- Selected widget
- Focused process
- Active page
- Command palette state
- Current theme
- Current layout
- Pause state
- Reduced-motion preference

Application state must not contain terminal-specific rendering objects.

---

## History

The history layer stores recent metric samples.

Initial metrics may include:

- CPU usage
- Memory usage
- GPU usage
- Network download
- Network upload
- Temperature

History must have bounded memory usage.

Each metric should define:

- Maximum number of samples
- Sampling interval
- Value range
- Missing-data behavior

The history layer should provide data suitable for charts and insights without knowing how charts are rendered.

---

## Animation Engine

The animation engine creates smooth visual transitions between previous and current values.

It may support:

- Numeric interpolation
- Meter interpolation
- Chart transitions
- Selection transitions
- Pulse intensity
- Widget entrance and focus states

Collection frequency and rendering frequency must remain independent.

Example:

```text
Metric collection: 1–4 Hz
Interface rendering: up to 30 FPS
```

Animations must never modify the factual system snapshot.

They should produce presentation values derived from application state.

A reduced-motion mode must be supported.

---

## Layout

The layout layer decides where widgets are positioned.

It is responsible for:

- Terminal-size adaptation
- Compact layouts
- Widget placement
- Minimum dimensions
- Hidden widgets
- Future configurable arrangements

Layout logic must remain separate from widget content.

A widget should receive an area and render inside it without deciding the complete dashboard layout.

---

## Widgets

Widgets present one focused part of the system state.

Initial widgets may include:

- Overview
- CPU
- Memory
- GPU
- Processes
- Storage
- Network
- History
- System Pulse
- Event Timeline
- Footer
- Command Palette

Each widget should:

- Receive prepared state
- Render within a provided area
- Avoid direct system collection
- Avoid controlling unrelated widgets
- Handle small areas safely
- Provide a predictable minimum size

Do not introduce a generic widget trait until multiple widgets demonstrate a real shared contract.

Prefer concrete modules before premature abstraction.

---

## Renderer

The renderer converts widget output into terminal content.

The initial renderer uses Ratatui and Crossterm.

Rendering should use semantic theme tokens rather than hard-coded colors.

Future rendering capabilities may include:

```text
Standard ANSI
Unicode and Braille
Kitty Graphics Protocol
```

Advanced backends must remain optional.

SysMon must continue to provide a functional interface in standard terminals.

---

## Terminal Backend

The terminal backend is responsible for:

- Entering and leaving raw mode
- Alternate screen management
- Keyboard events
- Resize events
- Capability detection
- Frame presentation
- Terminal restoration after failures

Terminal cleanup must remain protected by RAII.

A panic or early return should not leave the user's terminal in an unusable state.

---

## Performance Principles

SysMon is itself a system monitor and must avoid becoming a meaningful source of system load.

Guidelines:

- Do not collect every metric on every rendered frame
- Cache expensive results
- Bound all history buffers
- Avoid unnecessary allocations in hot rendering paths
- Avoid cloning large snapshots without need
- Measure before introducing complex optimization

---

## Testing Strategy

Tests should cover:

- Metric parsing
- Invalid and unavailable metric handling
- History limits
- Threshold calculations
- Animation interpolation
- Layout behavior at representative terminal sizes
- Widget rendering with Ratatui TestBackend

Refactors must preserve existing behavior unless a change is explicitly documented.

---

## Architectural Constraints

- Collectors never render UI
- Widgets never invoke operating-system commands
- Themes never contain collection logic
- Animation state never replaces factual metric state
- Rendering backends remain optional implementation details
- New abstractions require demonstrated use cases
- Existing functionality must be preserved during structural refactors