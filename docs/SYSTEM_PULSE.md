# System Pulse

## Purpose

System Pulse is a real-time ambient visualization of the current behavior of the machine.

It combines multiple metrics into a single animated representation that communicates system activity through rhythm, shape, density and motion.

It is not intended to replace detailed metrics.

It provides immediate visual context.

---

## Design Goal

A user should be able to look at System Pulse and quickly perceive whether the system is:

- calm;
- active;
- under sustained load;
- experiencing a spike;
- transferring significant data;
- using the GPU;
- returning to a stable state.

The visualization should be recognizable as a core part of SysMon.

---

## Inputs

The initial version should use a limited and understandable set of inputs.

Suggested inputs:

```text
CPU usage
Memory usage
GPU usage
Network throughput
Load average
Recent CPU delta
```

Possible future inputs:

```text
Disk activity
Temperature
Memory pressure
Process churn
Context switches
Power usage
Fan speed
```

Do not add collectors only to make the Pulse more visually complex.

---

## Normalized Pulse State

System Pulse should consume normalized values.

Conceptual representation:

```rust
pub struct PulseInput {
    pub cpu: f32,
    pub memory: f32,
    pub gpu: Option<f32>,
    pub network: f32,
    pub load: f32,
    pub volatility: f32,
}
```

Each value should be normalized to a predictable range, preferably:

```text
0.0 to 1.0
```

The final Rust representation may differ.

---

## Derived Properties

System Pulse may derive:

```text
Intensity
Rhythm
Radius
Density
Turbulence
Rotation
Particle emission
Stability
Color emphasis
```

Suggested relationships:

### Intensity

Represents combined system activity.

CPU and load may have the strongest influence.

### Rhythm

Controls breathing or pulse speed.

Higher activity creates a faster rhythm.

### Radius

Represents general resource pressure.

Memory and sustained load may increase average radius.

### Density

Represents how visually full the Pulse appears.

GPU and CPU activity may increase density.

### Turbulence

Represents short-term instability.

Rapid metric changes increase turbulence.

### Flow

Represents network movement.

Network activity may create directional trails or outward movement.

---

## Suggested Combination

An initial weighted intensity could conceptually follow:

```text
intensity =
    cpu * 0.35
  + memory * 0.20
  + gpu * 0.20
  + network * 0.10
  + load * 0.15
```

If GPU data is unavailable, its weight should be redistributed or excluded safely.

These values are a starting point, not a permanent mathematical contract.

They must be tested visually and adjusted based on behavior.

---

## Pulse States

### Calm

Typical characteristics:

- low intensity;
- slow breathing;
- small radius;
- low density;
- minimal turbulence;
- subtle color.

### Active

Typical characteristics:

- moderate rhythm;
- medium radius;
- visible internal movement;
- stable animation;
- moderate density.

### Busy

Typical characteristics:

- faster rhythm;
- increased density;
- stronger color;
- more active outer boundary;
- clearer metric reaction.

### Critical

Typical characteristics:

- high intensity;
- warning or danger semantic color;
- clear but readable visual emphasis;
- no rapid flashing;
- no visually chaotic behavior.

Critical state should remain informative rather than alarming without context.

---

## Breathing Model

The base Pulse can use a periodic breathing function.

Conceptual model:

```text
breath = sin(time × speed)
```

The output should be normalized and combined with metric intensity.

Possible interpretation:

```text
radius = base_radius + breath × amplitude
```

Where:

- `speed` depends on system activity;
- `amplitude` depends on intensity;
- reduced motion lowers or removes oscillation.

The implementation does not need physically accurate simulation.

It needs predictable and visually meaningful behavior.

---

## Volatility

Volatility represents how quickly the system state changes.

Conceptual input:

```text
abs(current_cpu - previous_cpu)
abs(current_gpu - previous_gpu)
change in network throughput
```

High volatility may produce:

- temporary boundary distortion;
- short-lived density changes;
- faster easing;
- a Timeline event when thresholds are crossed.

Volatility should decay smoothly.

---

## Unicode Renderer

The baseline Pulse should be renderable using terminal-native characters.

Potential techniques:

- Braille cells;
- block characters;
- shaded Unicode characters;
- polar point sampling;
- animated ring fields;
- spark-like particles.

The renderer should produce a deterministic grid for a given:

- input state;
- time value;
- width;
- height;
- theme;
- reduced-motion setting.

---

## Braille Representation

Braille characters provide a denser logical pixel grid.

Each terminal cell represents a matrix of dots.

This can support:

- circles;
- rings;
- waves;
- particle fields;
- flow;
- noise-like distortion.

Braille generation should be implemented separately from metric interpretation.

Conceptual separation:

```text
Pulse Input
    ↓
Pulse Model
    ↓
Point Field
    ↓
Braille Conversion
    ↓
Terminal Rendering
```

---

## Animation

System Pulse animation must be independent from metric collection.

New metric snapshots update target values.

The animation engine smoothly approaches those targets.

Example:

```text
Collection: every 1 second
Pulse animation: up to 30 frames per second
```

Animation should remain stable when collection intervals change.

---

## Reduced Motion

When reduced motion is active:

- breathing amplitude should be minimal or disabled;
- turbulence should be disabled;
- particles should be static or absent;
- metric state should still be represented;
- color and density may continue to communicate intensity;
- no essential information should be lost.

---

## Small-Terminal Behavior

When insufficient space exists:

1. Reduce detail.
2. Reduce particle density.
3. Render a compact Pulse.
4. Replace it with a small activity indicator if necessary.
5. Never compromise essential metric panels.

System Pulse is important, but not more important than usability.

---

## Interaction

The initial Pulse may remain non-interactive.

Possible future interactions:

- focus to show contributing metrics;
- toggle between visual modes;
- inspect current intensity composition;
- select reduced visual quality;
- open Pulse settings.

Do not add interaction before the base visualization is stable.

---

## Performance

Pulse generation must avoid excessive allocation.

Possible strategies:

- reuse buffers;
- precompute geometry;
- use bounded particle collections;
- update only required cells;
- limit maximum detail by terminal size;
- lower frame rate when idle;
- simplify under reduced motion.

Measure actual resource usage before implementing complex optimization.

---

## Testing

Tests should cover:

- normalized input clamping;
- missing GPU behavior;
- deterministic frames;
- calm, active and busy states;
- reduced-motion output;
- small dimensions;
- zero dimensions;
- intensity calculation;
- volatility decay;
- stable Braille conversion;
- no panic with unavailable metrics.

---

## Out of Scope for Initial Version

The first System Pulse should not require:

- Kitty Graphics;
- image rendering;
- physically accurate fluid simulation;
- GPU shaders;
- external processes;
- machine learning;
- plugin input;
- user-created equations.

The first goal is a meaningful, beautiful and efficient terminal-native Pulse.
