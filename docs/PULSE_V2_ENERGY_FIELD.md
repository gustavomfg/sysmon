# Pulse — Energy Field Renderer

## 1. Status

Implemented.

This document describes the active Pulse renderer used by the default dashboard.
The energy-field pipeline is the single supported Pulse implementation.

---

## 2. Vision

Pulse represents the state of the computer as an emergent computational phenomenon.

It must not look like:

- a traditional system graph;
- an oscilloscope;
- an equalizer;
- a collection of curves;
- explicit streamlines;
- animated ribbons;
- a decorative particle effect.

It should resemble a living field of computational energy whose visible structures emerge from density, flow, convergence, texture, and illumination.

The intended reaction is:

> “How is this being rendered inside a terminal?”

The Pulse should communicate the general condition of the machine before the user reads any numerical metric.

---

## 3. Core Principle

The energy field is the primary representation.

The energy field becomes the primary representation.

Motion, filaments, convergence, and luminous structures emerge from that field.

Conceptually:

```text
System Metrics
      ↓
Physical Parameters
      ↓
Scalar Energy Field
      ↓
Deterministic Noise Field
      ↓
Flow-Guided Texture
      ↓
Energy Organization
      ↓
Adaptive Illumination
      ↓
Braille / Unicode Rasterization

---

## 4. Design Principles

Pulse is built around a small set of immutable architectural principles.

### Emergence

Visible structures must emerge from the state of the energy field.

The renderer must never draw visual features directly simply because they are expected to exist.

Filaments, convergence regions, luminous peaks and transitions should all arise from interactions inside the simulation.

### Determinism

Given identical inputs, time and viewport dimensions, the renderer must always produce identical output.

No per-frame randomness is permitted.

### Continuity

The phenomenon must evolve continuously.

Temporal evolution should never appear as disconnected frames or reconstructed patterns.

### Hierarchy

Every visual element must have a clear perceptual role.

The renderer should naturally guide the observer's attention toward the dominant energetic region.

### Negative Space

Empty regions are part of the composition.

The renderer should intentionally preserve silence between energetic structures.

### Performance

Visual quality must always respect the constraints of terminal rendering.

The renderer must remain lightweight enough for continuous animation.

---

## 5. Architecture

Pulse is organized as a pipeline of independent stages.

Each stage transforms the previous representation into a richer description of the phenomenon.

```text
System Metrics
        │
        ▼
Physical Parameters
        │
        ▼
Scalar Energy Field
        │
        ▼
Deterministic Noise
        │
        ▼
Flow-Guided Organization
        │
        ▼
Energy Density
        │
        ▼
Adaptive Lighting
        │
        ▼
Braille / Unicode Rasterization
```

Each stage owns a single responsibility.

No stage should contain rendering logic that belongs to another.

---

## 6. Scalar Energy Field

The Scalar Energy Field is the fundamental representation of the phenomenon.

It describes how energy is distributed throughout the virtual space.

Every subsequent stage derives its information from this field.

The field should be continuous, bounded and deterministic.

It should not contain explicit graphical concepts such as:

- filaments;
- particles;
- ribbons;
- curves.

Instead, it represents only physical quantities.

Suggested properties include:

- energy;
- pressure;
- convergence;
- local gradient;
- depth;
- stability.

The exact implementation is intentionally left open.

---

## 7. Energy Organization

The renderer does not organize visible structures directly.

Instead, it organizes energy.

Regions of high coherence naturally become visible after transport and lighting.

This inversion is one of the defining characteristics of Pulse.

The renderer should never answer:

"Where should I draw a filament?"

Instead it should answer:

"How is energy distributed?"

---

## 8. Flow

The flow field is responsible only for transporting information.

It should not create visual structures by itself.

Its responsibilities include:

- transport direction;
- advection;
- continuity;
- gradual deformation;
- large-scale organization.

The flow should influence the energy field without explicitly drawing trajectories.

---

## 9. Texture

Texture represents the internal organization of the energy field.

It exists to reveal structure.

Not to decorate the phenomenon.

The preferred approach is a deterministic flow-guided texture inspired by scientific visualization techniques.

The exact algorithm is intentionally unspecified.

Possible implementations include:

- simplified Line Integral Convolution;
- directional convolution;
- oriented spot accumulation;
- other deterministic flow-aligned techniques.

The chosen solution must prioritize perceptual quality over algorithmic complexity.

---

## 10. Lighting

Lighting is responsible for translating energy into perception.

It does not generate energy.

It only reveals existing structure.

Lighting should consider:

- local energy;
- depth;
- convergence;
- neighborhood contrast;
- semantic health state.

Artificial glow, halos or decorative bloom are outside the scope of the renderer.

---

## 11. Rasterization

Rasterization is the final stage.

It converts the continuous internal representation into terminal cells.

It must remain completely independent from the simulation.

Its responsibilities include:

- Braille generation;
- Unicode fallback;
- aspect ratio correction;
- clipping;
- terminal compatibility.

Simulation code must never depend on rasterization details.

---

## 12. System Health

System metrics influence the phenomenon indirectly.

Metrics should modify physical parameters rather than graphical parameters.

For example:

- CPU influences activity.
- Memory influences compression.
- GPU influences richness.
- Network influences directional transport.
- Load influences pressure.
- Volatility influences instability.

The renderer should never switch between predefined visual effects.

The phenomenon remains the same across all health states.

Only its behavior changes.

---

## 13. Success Criteria

The current Pulse implementation is successful when:

- the phenomenon no longer resembles animated lines;
- visible structures emerge naturally from the energy field;
- the observer immediately identifies a dominant energetic region;
- negative space remains intentional;
- the phenomenon remains recognizable across all viewport sizes;
- Reduced Motion preserves the composition;
- deterministic rendering is maintained;
- the renderer remains performant enough for continuous animation.

Most importantly:

The renderer should feel like observing a living computational phenomenon rather than watching a graphical animation.
