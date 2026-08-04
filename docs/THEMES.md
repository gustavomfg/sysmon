# Themes

## Purpose

SysMon themes define the visual language of the terminal interface.

A theme must provide consistent semantic colors for widgets, metrics, alerts, selections and animations.

Widgets must not hard-code visual colors.

They should request semantic tokens from the active theme.

---

## Design Principles

Themes should preserve:

- Readability
- Strong contrast
- Clear metric hierarchy
- Consistency across widgets
- Compatibility with dark terminal backgrounds
- Graceful degradation in limited terminals
- Accessibility for users with reduced color perception

Color must not be the only way to communicate important state.

Warnings and errors should also use labels, symbols or emphasis.

---

## Semantic Tokens

A theme should define colors by meaning rather than by individual component.

Suggested tokens:

```text
background
surface
surface_alt

text
text_muted

border
border_focused

primary
secondary
accent

success
warning
danger
info

chart_primary
chart_secondary
chart_grid

selection_background
selection_text

pulse_low
pulse_medium
pulse_high
```

---

## Default Theme

The initial default theme should provide a modern dark interface with purple and cyan accents.

Working name:

```text
Pulse
```

Suggested colors:

```text
background           #080A0F
surface              #0D1018
surface_alt          #121622

text                 #E8EAF0
text_muted           #747B8D

border               #2A3040
border_focused       #A855F7

primary              #A855F7
secondary            #22D3EE
accent               #7C3AED

success              #22C55E
warning              #F59E0B
danger               #EF4444
info                 #38BDF8

chart_primary        #A855F7
chart_secondary      #22D3EE
chart_grid           #252A38

selection_background #312E81
selection_text       #FFFFFF

pulse_low            #6D28D9
pulse_medium         #A855F7
pulse_high           #22D3EE
```

These values are an initial visual direction and may be adjusted after implementation testing.

---

## Usage Thresholds

Metric colors should follow semantic thresholds rather than widget-specific rules.

Default proposal:

```text
Normal     0–59%
Warning    60–84%
Critical   85–100%
```

Some metrics require different thresholds.

Examples:

- Temperature depends on device type
- Disk space may warn based on remaining capacity
- Network traffic should not be considered unhealthy only because it is high
- Load average should consider CPU count

Thresholds belong to metric interpretation or configuration, not directly to the theme.

The theme only determines how normal, warning and critical states appear.

---

## Theme Presets

Future built-in presets may include:

```text
Pulse
Catppuccin Mocha
Tokyo Night
Nord
Dracula
Gruvbox Dark
Monochrome
High Contrast
```

Preset support should be introduced only after the theme engine is stable.

Do not implement every preset during the first theme-system task.

---

## Terminal Color Capabilities

Themes should account for different terminal capabilities:

```text
True color
256 colors
16 colors
Monochrome
```

The first implementation may prioritize true color and ANSI compatibility.

Fallback behavior must preserve meaning even when exact colors cannot be reproduced.

---

## Widget Rules

Widgets should use semantic roles consistently.

Examples:

```text
Widget title          primary or secondary
Focused border        border_focused
Inactive border       border
Main value            text
Supporting label      text_muted
Healthy metric        success
Warning metric        warning
Critical metric       danger
Primary chart         chart_primary
Secondary chart       chart_secondary
```

A widget must not introduce an isolated palette without a documented reason.

---

## System Pulse

The System Pulse visualization should derive its appearance from both the theme and current system intensity.

Conceptual behavior:

```text
Low activity:
- pulse_low
- slower motion
- lower density

Moderate activity:
- pulse_medium
- normal motion
- medium density

High activity:
- pulse_high
- faster motion
- greater density
```

Animation intensity must respect reduced-motion settings.

Avoid excessive flickering.

---

## Accessibility

Themes should support:

- High-contrast text
- Distinguishable focused states
- Reduced reliance on red and green alone
- Reduced-motion mode
- Monochrome fallback
- Symbols or labels for warning states

Examples:

```text
✓ Normal
! Warning
× Critical
```

Avoid rapid flashing.

---

## Theme Boundaries

Themes may define:

- Colors
- Text emphasis
- Border appearance
- Chart appearance
- Selection appearance
- Pulse palette

Themes must not define:

- Metric collection intervals
- Widget business logic
- Dashboard data
- Terminal capability detection
- Navigation behavior