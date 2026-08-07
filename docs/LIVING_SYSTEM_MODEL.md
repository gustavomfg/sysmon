# Living System Model

## Concept

SysMon models a computer as a living system.

Subsystems behave like organs.

The entire system produces a heartbeat.

This heartbeat is represented by System Pulse.

System Pulse is not another graph.

It is the visual expression of the overall health of the machine.

---

## Organs

### CPU

Represents computational activity.

High sustained load increases physiological stress.

---

### Memory

Represents working memory.

Memory pressure is more important than raw allocation.

High pressure affects system stability.

---

### GPU

Represents graphics and parallel processing activity.

Heavy GPU workloads contribute to overall stress but do not dominate it.

---

### Storage

Represents persistent memory.

Sustained I/O affects responsiveness.

Capacity represents long-term health.

Activity represents short-term stress.

---

### Network

Represents communication.

Traffic changes the rhythm of the organism without necessarily indicating poor health.

---

### Temperature

Represents metabolism.

Thermal pressure influences overall system health regardless of utilization.

---

## System Pulse

System Pulse does not visualize individual metrics.

Instead, it visualizes the physiological condition produced by all subsystems together.

Subsystems influence health.

Health produces Pulse.

This distinction is fundamental.

---

## Health

Health is not equal to utilization.

Examples:

High CPU while compiling software may be healthy.

Low CPU with memory exhaustion is unhealthy.

Moderate utilization with excessive temperatures is unhealthy.

Storage saturation during backups is expected.

Health depends on context.

Future versions may expose an explicit Health Engine responsible for converting subsystem conditions into Pulse behavior.

---

## Design Rule

Subsystems explain.

Pulse summarizes.

Metrics diagnose.

Pulse communicates.

## Health Interpretation

The Dashboard V2 now introduces a small Health Engine between metric models
and the Pulse boundary:

```text
Metric Models / History
          ↓
Component Health
          ↓
System Health
          ↓
Pulse input
```

The engine keeps activity and pressure separate. High CPU, GPU or network
activity is not automatically a health problem; memory pressure, storage
capacity pressure and thermal pressure are interpreted independently.

The interpretation is bounded and temporal. Rising pressure responds faster
than recovery, so a single sample cannot make the organism jump directly from
calm to critical or back again. Diagnostic instruments continue to consume
the objective metric models unchanged.
