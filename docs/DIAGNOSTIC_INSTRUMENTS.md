# Diagnostic Instruments

## Philosophy

Diagnostic instruments exist to answer objective questions.

They are intentionally conservative.

They should resemble professional monitoring tools rather than decorative dashboard widgets.

Developers already understand these visual languages.

SysMon should leverage that familiarity.

---

# CPU

Question answered:

"How busy is the processor?"

Primary representation:

- utilization;
- historical trend;
- frequency;
- temperature.

Purpose:

Show computational load immediately.

---

# Memory

Question answered:

"How much working memory is available?"

Primary representation:

- used;
- total;
- pressure;
- swap.

Purpose:

Communicate memory pressure, not animation.

---

# GPU

Question answered:

"How busy is the graphics processor?"

Primary representation:

- utilization;
- VRAM;
- temperature;
- frequency.

Purpose:

Summarize graphics workload.

---

# Storage

Question answered:

"How healthy are storage devices?"

Primary representation:

- capacity;
- utilization;
- read throughput;
- write throughput.

Purpose:

Separate occupancy from activity.

---

# Network

Question answered:

"How much traffic exists?"

Primary representation:

- download;
- upload;
- throughput history.

Purpose:

Communicate traffic flow.

---

# Temperatures

Question answered:

"Which components are thermally stressed?"

Primary representation:

- sensor;
- current temperature;
- thermal state.

Purpose:

Provide immediate thermal awareness.

---

# Processes

Question answered:

"What is consuming system resources?"

Primary representation:

- process;
- CPU;
- memory.

Purpose:

Identify responsibility.

---

# System Context

Question answered:

"What machine am I observing?"

Primary representation:

- uptime;
- physical cores;
- logical cores;
- total memory;
- operating context.

Purpose:

Provide environmental information.

---

# Visual Rules

Every diagnostic instrument follows the same hierarchy.

Title

↓

Primary value

↓

Visualization

↓

Context

↓

Labels

Diagnostic widgets should never compete with System Pulse.

They exist to explain it.

---

# Design Principles

Diagnostic instruments prioritize:

- readability;
- consistency;
- density;
- precision;
- familiarity.

They do not prioritize novelty.

Users should recognize these instruments immediately without learning a new visual language.
