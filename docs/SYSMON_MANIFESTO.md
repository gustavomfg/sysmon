# SysMon Manifesto

## Purpose

SysMon is not another terminal dashboard.

SysMon is a system monitor.

Its purpose is not to impress with visual effects or reinvent well-established monitoring conventions.

Its purpose is to help developers understand the current state of their machine with the smallest possible cognitive effort.

---

## Philosophy

Every design decision must answer one question:

> Does this help the user understand the system faster?

If the answer is no, the feature should not exist.

---

## Identity

SysMon has only one expressive component.

System Pulse.

Everything else is diagnostic.

CPU, Memory, GPU, Storage, Network, Temperatures and Processes are instruments.

System Pulse is the identity.

---

## Beauty

Beauty is not the objective.

Clarity is.

Beauty emerges naturally from:

- consistency;
- hierarchy;
- density;
- rhythm;
- precision.

The only intentionally expressive component is System Pulse.

---

## Familiarity

Developers already know how to read:

- CPU graphs;
- memory meters;
- process tables;
- storage usage;
- network throughput.

SysMon should not reinvent these conventions simply to appear different.

Innovation belongs where it creates value.

That place is System Pulse.

---

## Cognitive Flow

The intended interaction is:

Observe Pulse

↓

Detect that something changed

↓

Inspect diagnostics

↓

Understand the cause

↓

Return to Pulse

The Pulse communicates health.

Metrics communicate causes.

---

## Information Hierarchy

System Pulse

↓

Primary diagnostics

↓

Secondary diagnostics

↓

Context

↓

Labels

Only one component should dominate visual attention.

That component is always the Pulse.

---

## Minimalism

Every character on screen must justify its existence.

Unused space is acceptable.

Meaningless space is not.

Every visualization must improve understanding.

If a visualization is beautiful but slower to interpret than a number, it should be reconsidered.

---

## Success

SysMon succeeds when a developer can understand the overall condition of the machine before reading individual metrics.

If users first notice the Pulse, then immediately identify the responsible subsystem through familiar diagnostic instruments, the design has achieved its purpose.
