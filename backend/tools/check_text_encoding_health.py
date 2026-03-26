#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Repository text encoding health check."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Iterable, List, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DEFAULT_ROOTS = (
    Path("frontend/src"),
    Path("backend/app"),
    Path("backend/tests"),
    Path("docs"),
)
TEXT_EXTENSIONS = {
    ".py", ".ts", ".tsx", ".js", ".jsx", ".md", ".json", ".yaml", ".yml", ".toml",
    ".ini", ".cfg", ".conf", ".sql", ".ps1", ".bat", ".sh", ".txt", ".html", ".css",
}
SKIP_PARTS = {
    ".git", ".venv", "node_modules", "__pycache__", "dist", "build", "coverage",
    ".next", ".nuxt", ".cache", "static", "embedding", "logs", ".codex-tmp",
    ".claude", ".spec-workflow", ".zcf", "images", "data",
}
QMARK_PATTERN = re.compile(r"\?{3,}")
MOJIBAKE_CHARS = {chr(code) for code in (0x00C3, 0x00C2, 0x00E6, 0x00E5, 0x00E4, 0x00E7, 0x00E9, 0x00E8, 0x00EA, 0x00EF, 0x00F0, 0x00F8, 0x00E3, 0x00E2)} | {chr(0xFFFD)}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Scan source files for encoding corruption")
    parser.add_argument(
        "--root",
        action="append",
        default=[],
        help="Additional root to scan (relative to repo root, repeatable)",
    )
    parser.add_argument(
        "--strict-qmark",
        action="store_true",
        help="Treat double question marks as suspicious too",
    )
    return parser.parse_args()


def resolve_roots(extra_roots: Sequence[str]) -> List[Path]:
    roots = [REPO_ROOT / item for item in DEFAULT_ROOTS]
    for raw in extra_roots:
        path = (REPO_ROOT / raw).resolve()
        if path not in roots:
            roots.append(path)
    return roots


def is_text_file(path: Path) -> bool:
    return path.suffix.lower() in TEXT_EXTENSIONS or path.name.lower() in {"dockerfile", "makefile"}


def should_skip(path: Path) -> bool:
    return any(part in SKIP_PARTS for part in path.parts)


def has_cjk(text: str) -> bool:
    return any(
        ("一" <= ch <= "鿿") or ("㐀" <= ch <= "䶿") or ch in "?????????????????????"
        for ch in text
    )


def detect_reasons(line: str, *, strict_qmark: bool) -> List[str]:
    reasons: List[str] = []
    qmark_pattern = re.compile(r"\?{2,}") if strict_qmark else QMARK_PATTERN
    sanitized_line = re.sub(r"\?\?|\?\.(?=[A-Za-z_(\[])", "", line)
    if qmark_pattern.search(sanitized_line):
        reasons.append("qmark")
    if any(0x80 <= ord(ch) <= 0x9F for ch in line):
        reasons.append("control")
    if sum(1 for ch in line if ch in MOJIBAKE_CHARS) >= 1:
        try:
            fixed = line.encode("latin1").decode("utf-8")
        except Exception:
            fixed = None
        if fixed and fixed != line and has_cjk(fixed):
            reasons.append("mojibake")
    return reasons


def iter_files(roots: Sequence[Path]) -> Iterable[Path]:
    seen: set[Path] = set()
    for root in roots:
        if not root.exists():
            continue
        candidates = [root] if root.is_file() else root.rglob("*")
        for path in candidates:
            if not path.is_file():
                continue
            rel = path.relative_to(REPO_ROOT) if path.is_absolute() else path
            if should_skip(rel):
                continue
            if not is_text_file(path):
                continue
            resolved = path.resolve()
            if resolved in seen:
                continue
            seen.add(resolved)
            yield path


def scan_file(path: Path, *, strict_qmark: bool) -> List[Tuple[int, List[str], str]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except Exception:
        return []
    findings: List[Tuple[int, List[str], str]] = []
    for idx, line in enumerate(lines, 1):
        reasons = detect_reasons(line, strict_qmark=strict_qmark)
        if reasons:
            findings.append((idx, reasons, line))
    return findings


def main() -> int:
    args = parse_args()
    roots = resolve_roots(args.root)
    findings_total = 0
    for path in iter_files(roots):
        findings = scan_file(path, strict_qmark=args.strict_qmark)
        if not findings:
            continue
        rel = path.relative_to(REPO_ROOT)
        print(f"\n{rel}")
        for line_no, reasons, content in findings:
            print(f"  L{line_no} [{",".join(reasons)}] {content}")
        findings_total += len(findings)
    if findings_total == 0:
        print("OK: no suspicious encoding issues found.")
        return 0
    print(f"\nFAIL: found {findings_total} suspicious encoding issue(s).")
    return 1


if __name__ == "__main__":
    sys.exit(main())
