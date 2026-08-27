SysMon is not another terminal dashboard.
It is a living system monitor.
The Pulse tells you how your machine feels.
The metrics tell you why.

System Pulse does not visualize metrics.

System Pulse visualizes the physiological condition produced by those metrics.

Metrics influence the organism.

The organism produces the Pulse.

This distinction is fundamental.

Memory pressure should not merely increase radius.

It should create the visual sensation of compression.

CPU saturation should accelerate the heartbeat.

Network activity should produce directional flow.

GPU load should increase energetic richness.

Storage activity should introduce localized turbulence.

Rapid state changes should temporarily destabilize the heartbeat before it settles again.

## Health Boundary

Pulse V2 receives an aggregate `SystemHealth` interpretation rather than
consulting CPU, memory, GPU or other instruments directly. The Health Engine
produces bounded activity, pressure, thermal pressure, instability, severity
and a dominant source. `PulseBehavior` then maps those aggregate signals to
the existing Pulse animation boundary.

This boundary keeps the Pulse expressive and singular while leaving metrics
objective and diagnostic. `PulseBehavior` maps aggregate signals to a calm
basal rhythm, activity energy, pressure tension and bounded turbulence. Its
attack/release controller makes entry responsive and recovery slower; the
Pulse geometry and renderer remain unchanged.

The concrete runtime path is:

```text
SystemHealth
     ↓
PulseBehavior
     ↓
PulseBehaviorEngine (attack / release)
     ↓
NormalizedPulseInput
     ↓
Pulse V2 animation and renderer
```

The input passed to the existing V2 field engine is a perceptual projection,
not a second metric pipeline. Activity is carried primarily by cadence and
energetic channels so a healthy workload can make the organism more active.
Pressure and thermal pressure are carried by tension/load channels so memory,
capacity and thermal constraints produce contraction and slower recovery. The
V2 field dynamics preserve this distinction before the existing organic
motion, transport, lighting and renderer stages.

The health and pressure scenarios are covered by deterministic unit tests. They
exercise the same Health Engine → PulseBehavior pipeline as Live mode and are
not fixtures used by Live Metrics.
