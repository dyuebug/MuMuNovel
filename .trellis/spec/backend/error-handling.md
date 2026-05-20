# Error Handling

> How backend errors are handled and returned.

---

## Overview

Backend error handling combines app-level global exception handlers in the
bootstrap layer, route-level use of `HTTPException` for expected request
failures, and logger-backed error reporting in infrastructure and services.

The project already translates some operational failures into structured 503
responses instead of leaking raw stack traces to clients.

---

## Global Exception Handling

- Central exception handlers are registered in
  `backend/app/bootstrap/app_factory.py`.
- `RequestValidationError` is converted into a `422` JSON response with a
  generic `detail` plus structured `errors`.
- Generic uncaught exceptions are logged with `exc_info=True`.
- Connection, timeout, and database availability failures are normalized to
  `503 Service Unavailable`.
- In debug mode, the response may expose the concrete exception message; in
  non-debug mode, responses are intentionally more generic.

---

## Route-Level Handling

- Use `HTTPException` for expected request-time failures such as
  authentication, missing user context, forbidden access, or resource-level
  validation.
- Prefer shared auth/access helpers instead of duplicating status logic.
- Keep route handlers thin; complex recovery or fallback logic belongs in
  services.
- Example:
  `backend/app/database.py` raises `HTTPException(status_code=401, ...)` when
  request user context is missing.

---

## Service and Infrastructure Errors

- Services and infrastructure modules typically log failures via
  `get_logger(__name__)`.
- Infrastructure code often logs before re-raising so the global handler still
  owns the HTTP response shape.
- Background task and persistence flows favor structured status/error capture
  over hard process crashes.
- Use focused helper functions where repeated exception-to-message conversion
  is needed, for example `app.utils.exception_message`.

---

## Error Message Style

- The current backend mixes Chinese user-facing messages with technical detail
  for logs and debug-only branches.
- Favor:
  - concise user-facing `detail`
  - structured auxiliary error data where clients need field-level handling
  - richer log output for operators and debugging
- Keep externally returned messages stable enough that frontend code and tests
  can assert them when necessary.

---

## Review Checklist

- Is this an expected client error (`4xx`) or an operational/server error
  (`5xx`)?
- Did you log the failure at the correct layer without duplicating noisy logs?
- If the change affects auth, DB readiness, or service availability, does the
  response shape still match existing frontend expectations?
- If you changed validation behavior, do API tests still assert the correct
  `detail` or `errors` shape?

---

## Examples

- Global handlers and readiness-related `503` mapping:
  `backend/app/bootstrap/app_factory.py`
- Auth/session dependency failures:
  `backend/app/database.py`
- Logged persistence and recovery errors:
  `backend/app/services/background_task_manager.py`

## Anti-Patterns

- Do not swallow exceptions silently in services.
- Do not return inconsistent ad hoc error payloads when the route already has a
  stable response pattern.
- Do not expose internal exception text to production clients unless the code
  path intentionally gates that behind debug behavior.
