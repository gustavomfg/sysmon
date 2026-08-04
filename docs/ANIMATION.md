# Animation System

## Purpose

Animation in SysMon communicates change, activity and interaction.

It must improve understanding without distracting from system information.

Animation is a presentation concern.

It must never alter factual metric state.

---

## Principles

### Meaningful

Every animation should communicate at least one of:

- metric change;
- system activity;
- focus;
- selection;
- state transition;
- warning;
- command feedback.

### Subtle

SysMon should not animate every visible element continuously.

Motion should preserve readability.

### Deterministic

Given the same input state and time, visual output should be predictable.

This improves testing and debugging.

### Interruptible

New data or user interaction may arrive before an animation completes.

Animations must safely retarget without visual jumps or invalid state.

### Optional

Reduced motion must remain a first-class behavior.

---

## Animation Categories

### Metric Tweens

Used for:

- CPU percentages;
- memory percentages;
- GPU usage;
- meters;
- numeric values;
- chart transitions.

### Interaction Transitions

Used for:

- focus changes;
- Command Palette;
- Process Search;
- Settings;
- selected entries;
- widget state changes.

### Ambient Animation

Used for:

- System Pulse;
- network flow;
- subtle active-state indicators.

Ambient animation must be limited and performance-aware.

### Event Emphasis

Used for:

- new Timeline events;
- threshold crossings;
- successful exports;
- unavailable commands;
- configuration changes.

---

## Time Model

Animation updates should use elapsed time rather than frame counts.

Conceptual update:

```text
progress = elapsed / duration
```

Progress should be clamped:

```text
0.0 to 1.0
```

This keeps behavior consistent across different frame rates.

---

## Easing

Initial easing functions may include:

```text
Linear
Ease Out
Ease In Out
```

Use easing by purpose.

### Linear

Suitable for:

- continuous flow;
- rotation;
- progress with constant velocity.

### Ease Out

Suitable for:

- metric changes;
- focus movement;
- overlay appearance;
- values settling into a target.

### Ease In Out

Suitable for:

- breathing;
- reversible visual transitions;
- smooth ambient motion.

Avoid adding many easing functions without practical use.

---

## Metric Retargeting

When a new snapshot arrives during an active tween:

1. Use the current animated value as the new start.
2. Set the latest factual value as the new target.
3. Restart or adjust duration.
4. Avoid snapping backward to an old start value.

Conceptual example:

```text
Current animated value: 42
Old target: 60
New factual value: 75

New tween:
42 → 75
```

---

## Animation Durations

Suggested starting ranges:

```text
Metric tween          200–350 ms
Focus feedback        100–180 ms
Overlay transition    120–220 ms
Timeline emphasis     300–600 ms
System Pulse cycle    data-dependent
```

These values are initial guidance.

They should be tuned visually.

---

## Frame Rate

Target frame rate should be practical rather than absolute.

Possible target:

```text
30 FPS
```

The application may lower its effective frame rate when:

- no animation is active;
- reduced motion is enabled;
- the terminal is small;
- rendering performance is poor;
- the system is under heavy load.

Metric collection frequency must remain independent.

---

## System Pulse Animation

System Pulse may combine:

- periodic breathing;
- target intensity interpolation;
- volatility decay;
- optional rotation;
- particle lifetime;
- network flow.

These components should share a stable time source.

Pulse state must not depend on how many frames happened to render.

---

## Particle Model

Particles are optional and should begin as terminal-native points.

A minimal particle may contain:

```text
Position
Velocity
Lifetime
Intensity
Kind
```

Particle emission may derive from:

- network activity;
- CPU volatility;
- disk activity;
- GPU activity.

Particle count must be bounded.

The first Pulse version does not require particles.

---

## Overlay Animation

Terminal overlays should prioritize usability.

Possible behavior:

- instant layout placement;
- short emphasis on opening;
- selected row transition;
- no complex sliding geometry;
- no animation that delays input readiness.

Input must become available immediately.

---

## Timeline Animation

New Timeline entries may receive temporary emphasis.

Examples:

- brighter text;
- short accent marker;
- age-based fading;
- subtle movement-free highlight.

Timeline history should remain readable after emphasis ends.

---

## Reduced Motion

Reduced motion must:

- complete metric tweens immediately or near-immediately;
- disable breathing movement;
- disable particles;
- disable decorative transitions;
- preserve focus indication;
- preserve warnings;
- preserve state differences through static styling.

Reduced motion is not equivalent to disabling all feedback.

---

## Animation State

Animation state must remain separate from factual state.

Conceptual separation:

```text
System Snapshot
    factual metrics

Animation State
    current visual values
    target visual values
    timing
    active transitions
```

Exported snapshots must use factual state, not animated state.

---

## Scheduling

The runtime should determine the next meaningful wake-up time based on:

- metric collection deadline;
- active animation deadline;
- input availability;
- terminal resize;
- pending redraw.

Avoid a permanent busy loop.

---

## Performance

Guidelines:

- reuse buffers where practical;
- avoid cloning complete snapshots per frame;
- bound particle counts;
- avoid allocating strings repeatedly in hot paths;
- stop animation scheduling when nothing is moving;
- profile before introducing unsafe optimization.

---

## Testing

Tests should use explicit elapsed durations.

Avoid sleeping in unit tests.

Test cases should include:

- start;
- midpoint;
- completion;
- zero duration;
- invalid values;
- retargeting;
- reduced motion;
- long elapsed intervals;
- clamping;
- deterministic Pulse phase;
- inactive animation detection.

---

## Out of Scope

The initial animation system does not require:

- physics engines;
- shader languages;
- GPU acceleration;
- external animation libraries;
- physically correct particle simulation;
- frame-perfect synchronization with terminal refresh rate.

The objective is clear, efficient and meaningful terminal motion.
