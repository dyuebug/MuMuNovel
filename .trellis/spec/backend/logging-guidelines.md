# Logging Guidelines

> Structured logging and log-level conventions for the backend.

---

## Overview

The backend uses a centralized logger setup in `backend/app/logger.py`.
Logging follows a Uvicorn-like console format, supports optional rotating file
logs, and suppresses overly noisy third-party debug output by default.

---

## Logger Setup

- Configure logging through `setup_logging(...)` in
  `backend/app/logger.py`.
- Get module loggers through `get_logger(__name__)`; do not build ad hoc
  logger naming schemes.
- Application bootstrap wires logging before app creation in
  `backend/app/bootstrap/app_factory.py`.
- Log file output is optional and uses `RotatingFileHandler` with UTF-8
  encoding.

---

## Formatting and Context

- Console output uses the custom `UvicornFormatter`.
- When available, `request_id` is included in the log line; middleware is part
  of that context chain.
- The format is intentionally compact: level, logger name, optional request
  id, message.
- Prefer adding structured context through logger arguments or nearby metadata
  rather than hand-building huge string payloads everywhere.

---

## Log Levels

- `INFO` for normal lifecycle and operational milestones.
- `WARNING` for degraded-but-recoverable behavior, deprecated paths, or failed
  cleanup that the process can survive.
- `ERROR` for request, persistence, rollback, or background-task failures that
  need attention.
- Many third-party libraries are pinned to `WARNING` to avoid log spam; if you
  lower those levels, justify it carefully.

---

## What to Log

- App startup configuration outcomes and important lifecycle milestones.
- Request validation failures and uncaught exceptions at the app boundary.
- Database session rollback/close failures.
- Background-task persistence, recovery, and stream-processing issues.
- Avoid logging secrets such as raw API keys or sensitive credential payloads.

---

## Review Checklist

- Did you use `get_logger(__name__)` instead of a one-off logger?
- Is the level appropriate for the operational impact?
- Are you logging enough context to debug the issue without leaking secrets?
- If this path is high-frequency, will the new log become noise?

---

## Examples

- Central setup and formatter:
  `backend/app/logger.py`
- App-boundary exception logging:
  `backend/app/bootstrap/app_factory.py`
- Database session lifecycle error logs:
  `backend/app/database.py`
- Background-task persistence and recovery logs:
  `backend/app/services/background_task_manager.py`

## Anti-Patterns

- Do not log raw secrets, tokens, or credential blobs.
- Do not add noisy per-request info logs in hot paths without a clear ops need.
- Do not bypass the shared logger setup with module-local custom formatting.
