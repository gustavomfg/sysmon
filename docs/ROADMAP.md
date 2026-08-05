# Roadmap

## Phase 1 — Foundation

1. Widget architecture
2. Theme engine
3. State refactor

### Done when

- Dashboard responsibilities are separated into modules.
- Theme tokens replace hard-coded UI colors.
- Application state owns the current snapshot and presentation state.
- Existing behavior and tests are preserved.

---

## Phase 2 — Motion and Data

4. Animation engine
5. History engine
6. Layout manager

### Done when

- Metric values can transition smoothly.
- Histories are bounded and reusable.
- Collection and rendering schedules are independent.
- Layout decisions are separated from widget rendering.

---

## Phase 3 — Interaction Infrastructure

7. Infrastructure
8. Command Palette UI
9. Timeline
10. Focus Mode
11. Action Model

### Done when

- Input events are translated into explicit actions.
- Command Palette can open, filter, navigate and select commands.
- Timeline records relevant interactions.
- Focus can move between widgets.
- Commands that depend on future features are clearly unavailable.

---

## Phase 4 — Functional Commands

12. Theme switching
13. Widget management
14. Process search
15. Export snapshot
16. Config editor

### Done when

- Command Palette actions perform real operations.
- Users can switch between available themes.
- Widgets can be enabled, disabled and restored safely.
- Processes can be searched and focused.
- Current metrics can be exported.
- Configuration can be opened and edited through a supported workflow.
- ---

## 14. Architectural Roadmap

The Pulse V2 architecture is intentionally developed in layers.

Each stage introduces a new capability while preserving the contracts established by previous stages.

```text
Infrastructure
        ↓
Scalar Energy Field
        ↓
Deterministic Noise Field
        ↓
Gradient Flow Field
        ↓
Flow-Guided Transport
        ↓
Energy Organization
        ↓
Adaptive Lighting
        ↓
Terminal Rasterization
        ↓
Integration
```

### Infrastructure

Defines the engine, contracts, presets and internal data structures.

### Scalar Energy Field

Defines the physical energy distribution that represents the machine state.

### Deterministic Noise Field

Introduces procedural texture that will later be transported by the flow.

### Gradient Flow Field

Derives a continuous vector field from the scalar energy landscape.

### Flow-Guided Transport

Organizes procedural texture according to the vector field, revealing coherent structures.

### Emergent Structures

Transforms transported energy into recognizable filaments, convergence regions and coherent large-scale structures.

### Adaptive Lighting

Converts physical energy into perceptual hierarchy while preserving the phenomenon.

### Terminal Rasterization

Projects the continuous internal representation into Braille or Unicode cells.

### Integration

Connects the Pulse V2 renderer to the dashboard while preserving compatibility with Pulse V1 until the experimental implementation becomes the default.