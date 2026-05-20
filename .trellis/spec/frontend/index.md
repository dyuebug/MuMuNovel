# Frontend Development Guidelines

> Best practices for frontend development in this project.

---

## Overview

This directory contains project-specific frontend conventions extracted from
the current MuMuNovel codebase. The goal is to describe how frontend code
actually works today, including the real route tree, modular service/store
split, feature modules, and the current validation/test reality.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Module organization and file layout | Filled |
| [Component Guidelines](./component-guidelines.md) | Component patterns, props, composition | Filled |
| [Hook Guidelines](./hook-guidelines.md) | Custom hooks, data fetching patterns | Filled |
| [State Management](./state-management.md) | Local state, global state, server state | Filled |
| [Quality Guidelines](./quality-guidelines.md) | Code standards, forbidden patterns | Filled |
| [Type Safety](./type-safety.md) | Type patterns, validation | Filled |

---

## How to Use These Guidelines

Read these before changing frontend code:

1. [Directory Structure](./directory-structure.md)
2. [Component Guidelines](./component-guidelines.md)
3. [Hook Guidelines](./hook-guidelines.md)
4. [State Management](./state-management.md)
5. [Type Safety](./type-safety.md)
6. [Quality Guidelines](./quality-guidelines.md)

Also read `../guides/index.md` when the task touches multiple layers,
introduces repeated patterns, changes config/constants, or adds new payload
shapes across API/store/UI boundaries.

Useful repo-specific context before large frontend work:

- `frontend/CLAUDE.md`
- `frontend/src/pages/CLAUDE.md`
- `frontend/src/components/CLAUDE.md`
- `frontend/src/services/CLAUDE.md`
- `frontend/src/store/CLAUDE.md`

---

**Language**: All documentation should be written in **English**.
