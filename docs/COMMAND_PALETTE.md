# Command Palette

## Purpose

The Command Palette is the primary interaction hub of SysMon.

Its purpose is to provide a fast, keyboard-driven interface for executing commands, navigating the application and accessing future functionality without exposing complex keybindings.

The Command Palette should remain lightweight, discoverable and consistent across all phases of the project.

---

# Activation

Default shortcut:

```text
Ctrl+P
```

The palette appears as a centered modal overlay.

While open:

- it captures keyboard input;
- dashboard navigation is suspended;
- background metric collection continues unless the application is paused.

---

# Interaction

Supported controls:

```text
↑ / ↓     Navigate commands
Enter     Execute selected command
Esc       Close palette
Ctrl+P    Toggle palette
```

Typing filters the available commands in real time.

Matching should be case-insensitive.

---

# Command Lifecycle

Commands evolve together with the roadmap.

A command may be in one of three states:

## Available

The command is fully implemented and can be executed immediately.

## Planned

The command is visible but depends on functionality scheduled for a future phase.

Selecting a planned command must:

- explain why it is unavailable;
- record the attempt in the Timeline;
- never fail silently;
- never execute placeholder logic.

## Hidden

Commands that are not yet introduced by the roadmap should not appear.

---

# Current Commands

## Available

```text
Quit
Show Timeline
Toggle Focus Mode
```

## Planned

```text
Change Theme
Toggle GPU Widget
Restart Widget
Export Snapshot
Search Process
Open Settings
```

The list of available commands expands as new roadmap phases are completed.

---

# Search

Search filters commands by:

- command title
- aliases
- keywords

Example:

```text
theme
gpu
config
quit
search
```

Future fuzzy matching may be introduced, but exact substring matching is sufficient initially.

---

# Timeline Integration

Every command execution should create a Timeline entry.

Examples:

```text
Opened Command Palette
Closed Command Palette
Executed: Quit
Executed: Show Timeline
Unavailable: Change Theme
Unavailable: Export Snapshot
```

Timeline entries should remain concise and human-readable.

---

# Design Principles

The Command Palette should:

- prioritize keyboard navigation;
- remain responsive;
- avoid unnecessary visual noise;
- feel similar to modern developer tools;
- integrate naturally with Focus Mode.

The palette should never become a configuration screen.

Its responsibility is command execution.

---

# Future Commands

The following commands are expected to become available in future phases:

## Theme

- Change Theme

## Widgets

- Toggle GPU Widget
- Toggle Memory Widget
- Toggle Network Widget
- Restart Widget

## Data

- Export Snapshot
- Search Process

## Configuration

- Open Settings

Additional commands may be introduced as the project evolves.

---

# Out of Scope

The Command Palette must not:

- execute shell commands;
- act as a plugin manager;
- become a terminal emulator;
- replace widget-specific interactions;
- expose debugging-only commands in normal builds.

---

# Future Evolution

Future improvements may include:

- fuzzy search
- command history
- recently used commands
- command aliases
- configurable shortcuts
- command categories
- contextual commands
- command descriptions
- icons
- multi-step commands