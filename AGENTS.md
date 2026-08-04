# SysMon Development Guide

## Vision

SysMon is not intended to compete with btop.

Its goal is to provide the most modern terminal monitoring experience.

## Principles

- Modular widgets
- Smooth animations
- Extensible architecture
- Theme system
- Accessibility
- Performance first

## Architecture

Collector

↓

State

↓

Animation Engine

↓

Renderer

## Never

- Put rendering logic inside collectors.
- Put system collection inside widgets.
- Create widgets coupled together.