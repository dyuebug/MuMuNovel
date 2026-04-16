import json
from typing import Any


def _extract_message(value: Any, seen: set[int], depth: int) -> str:
    if value is None or depth > 4:
        return ""

    value_id = id(value)
    if value_id in seen:
        return ""
    seen.add(value_id)

    if isinstance(value, str):
        return value.strip()

    if isinstance(value, BaseException):
        for candidate in (
            getattr(value, "detail", None),
            getattr(value, "message", None),
            getattr(value, "error", None),
        ):
            message = _extract_message(candidate, seen, depth + 1)
            if message:
                return message

        for arg in getattr(value, "args", ()):
            message = _extract_message(arg, seen, depth + 1)
            if message:
                return message

        direct_message = str(value).strip()
        if direct_message:
            return direct_message

        for candidate in (getattr(value, "__cause__", None), getattr(value, "__context__", None)):
            message = _extract_message(candidate, seen, depth + 1)
            if message:
                return message

        return ""

    if isinstance(value, dict):
        for key in ("detail", "message", "error", "reason"):
            if key in value:
                message = _extract_message(value.get(key), seen, depth + 1)
                if message:
                    return message
        try:
            return json.dumps(value, ensure_ascii=False)
        except TypeError:
            return str(value).strip()

    if isinstance(value, (list, tuple, set)):
        parts: list[str] = []
        for item in value:
            message = _extract_message(item, seen, depth + 1)
            if message and message not in parts:
                parts.append(message)
        return "；".join(parts)

    return str(value).strip()


def extract_exception_message(exc: Any, fallback: str = "未知错误") -> str:
    message = _extract_message(exc, set(), 0)
    return message or fallback
