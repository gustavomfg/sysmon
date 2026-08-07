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
and a dominant source, then maps those aggregate signals to the existing
`NormalizedPulseInput` contract.

This boundary keeps the Pulse expressive and singular while leaving metrics
objective and diagnostic. The first mapping is intentionally conservative;
the Pulse geometry and renderer remain unchanged.
