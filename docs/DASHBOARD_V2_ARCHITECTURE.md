Dashboard V2 is not intended to be a visually impressive dashboard.

It is a modern diagnostic surface.

Its responsibility is to communicate the state of the machine with minimum cognitive effort.

Beauty exists to improve understanding.

Never to compete with information.

System Pulse

↓

Primary Diagnostics

↓

Secondary Diagnostics

↓

System Context

↓

Events

System Pulse answers:

"How is the machine behaving?"

CPU answers:

"Processor activity."

Memory answers:

"Memory pressure."

GPU answers:

"Graphics workload."

Network answers:

"Traffic."

Storage answers:

"I/O and capacity."

Processes answer:

"Who is responsible."

No diagnostic widget should attempt to summarize the entire machine.

Only the Pulse performs that role.

Visual language follows two distinct families.

Ambient Components

System Pulse

These components may use animation, gradients, density, rhythm and expressive rendering.

Diagnostic Components

CPU
Memory
GPU
Storage
Network
Processes

These components should resemble professional monitoring instruments.

Numbers first.

Decoration second.

Color has semantic meaning.

White

Primary values.

Muted Gray

Labels.

Purple

Primary activity.

Blue

Network and secondary activity.

Yellow

Warning.

Red

Critical.

No widget should introduce isolated colors.

Metrics should not attempt to impress.

They should disappear.

A user should never think:

"What a beautiful CPU widget."

Instead they should think:

"CPU is at 94%."

The Pulse is responsible for emotional perception.

Metrics are responsible for diagnosis.

## Health Engine Foundation

Dashboard V2 also exposes a collector-independent interpretation layer:

```text
Live Snapshot → Metric Models / History → HealthSnapshot
                                      ↓
                              HealthEngine
                                      ↓
                              SystemHealth
                                      ↓
                              PulseBehavior
                                      ↓
                       Pulse animation / renderer
```

`HealthSnapshot` contains normalized optional signals for CPU, Memory, GPU,
Storage, Network and Temperature. `SystemHealth` preserves continuous
activity, pressure, thermal pressure and instability values alongside a
discrete severity and dominant component. `HealthEngine` applies bounded
attack/release smoothing and uses centralized policy thresholds.

The layer never collects, renders or mutates metric history. The existing
diagnostic instrument path remains:

```text
Live Snapshot → Metric Models / History → Diagnostic Instruments
```

Only the aggregate `SystemHealth` crosses into the Pulse behavior boundary.
`PulseBehavior` maps activity, pressure, thermal pressure and instability to
continuous perceptual parameters, and `PulseBehaviorEngine` applies attack and
release before adapting them to the existing `NormalizedPulseInput` contract.
The Dashboard compositor does not read raw CPU, memory, GPU, network or
storage values to drive the Pulse. Severity and dominant source are available
for observability and future events, but do not create one animation per
component. This does not add Live fixtures or events.
