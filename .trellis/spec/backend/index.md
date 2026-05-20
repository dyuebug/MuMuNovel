# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

This directory contains project-specific backend conventions extracted from the
current MuMuNovel codebase. The goal is to describe how backend code actually
works today, including the current bootstrap/app split, shared-runtime task
patterns, and migration-sensitive areas.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Module organization and file layout | Filled |
| [Database Guidelines](./database-guidelines.md) | ORM patterns, queries, migrations | Filled |
| [Error Handling](./error-handling.md) | Error types, handling strategies | Filled |
| [Quality Guidelines](./quality-guidelines.md) | Code standards, forbidden patterns | Filled |
| [Logging Guidelines](./logging-guidelines.md) | Structured logging, log levels | Filled |

---

## How to Use These Guidelines

Read these before changing backend code:

1. [Directory Structure](./directory-structure.md)
2. [Database Guidelines](./database-guidelines.md)
3. [Error Handling](./error-handling.md)
4. [Logging Guidelines](./logging-guidelines.md)
5. [Quality Guidelines](./quality-guidelines.md)

Also read `../guides/index.md` when the task involves cross-layer changes,
repeated patterns, config changes, route/schema payload updates, or task/state
shape changes.

Useful repo-specific context before large backend work:

- `backend/CLAUDE.md`
- `backend/app/api/CLAUDE.md`
- `backend/app/services/CLAUDE.md`
- `backend/app/models/CLAUDE.md`
- `backend/app/schemas/CLAUDE.md`

---

**Language**: All documentation should be written in **English**.
