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