#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Alembic revision health check."""

from __future__ import annotations

import ast
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from alembic_versioning import ALEMBIC_VERSION_NUM_LENGTH

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
VERSION_ROOTS = {
    "postgres": REPO_ROOT / "backend/alembic/postgres/versions",
    "sqlite": REPO_ROOT / "backend/alembic/sqlite/versions",
}
MAX_REVISION_LENGTH = ALEMBIC_VERSION_NUM_LENGTH


@dataclass(frozen=True)
class MigrationInfo:
    path: Path
    revision: str | None
    down_revisions: tuple[str, ...] | None
    depends_on: tuple[str, ...]
    findings: tuple[str, ...]


@dataclass(frozen=True)
class ScopeReport:
    findings: tuple[str, ...]
    heads: tuple[str, ...]


def iter_version_files(root: Path) -> Iterable[Path]:
    if not root.exists():
        return

    for path in sorted(root.glob("*.py")):
        if path.name.startswith("__"):
            continue
        yield path


def get_assignment_name(node: ast.stmt) -> str | None:
    if isinstance(node, ast.Assign):
        for target in node.targets:
            if isinstance(target, ast.Name):
                return target.id
        return None
    if isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
        return node.target.id
    return None


def get_assignment_value(node: ast.stmt) -> ast.expr | None:
    if isinstance(node, ast.Assign):
        return node.value
    if isinstance(node, ast.AnnAssign):
        return node.value
    return None


def parse_revision_refs(raw_value: object, *, field_name: str) -> tuple[str, ...]:
    if raw_value is None:
        return ()
    if isinstance(raw_value, str):
        return (raw_value,)
    if isinstance(raw_value, (tuple, list)):
        values = tuple(raw_value)
        if not all(isinstance(item, str) for item in values):
            raise TypeError(f"{field_name} items must be strings")
        return values
    raise TypeError(f"{field_name} must be None, str, tuple[str, ...] or list[str]")


def parse_migration(path: Path) -> MigrationInfo:
    findings: list[str] = []
    try:
        source = path.read_text(encoding="utf-8")
    except Exception as exc:
        return MigrationInfo(
            path=path,
            revision=None,
            down_revisions=None,
            depends_on=(),
            findings=(f"  [read_error] {path.relative_to(REPO_ROOT)} -> {exc}",),
        )

    try:
        module = ast.parse(source, filename=str(path))
    except SyntaxError as exc:
        return MigrationInfo(
            path=path,
            revision=None,
            down_revisions=None,
            depends_on=(),
            findings=(f"  [syntax_error] {path.relative_to(REPO_ROOT)}:{exc.lineno} -> {exc.msg}",),
        )

    assignments: dict[str, object] = {}
    seen_fields: set[str] = set()
    for node in module.body:
        name = get_assignment_name(node)
        if name not in {"revision", "down_revision", "depends_on"}:
            continue
        seen_fields.add(name)
        value_node = get_assignment_value(node)
        if value_node is None:
            findings.append(f"  [invalid_{name}] {path.relative_to(REPO_ROOT)} -> missing value")
            continue
        try:
            assignments[name] = ast.literal_eval(value_node)
        except Exception as exc:
            findings.append(f"  [invalid_{name}] {path.relative_to(REPO_ROOT)} -> {exc}")

    revision: str | None = None
    if "revision" not in seen_fields:
        findings.append(f"  [missing_revision] {path.relative_to(REPO_ROOT)}")
    else:
        raw_revision = assignments.get("revision")
        if isinstance(raw_revision, str):
            revision = raw_revision
        elif raw_revision is not None:
            findings.append(
                f"  [invalid_revision] {path.relative_to(REPO_ROOT)} -> expected str, got {type(raw_revision).__name__}"
            )

    down_revisions: tuple[str, ...] | None = None
    if "down_revision" not in seen_fields:
        findings.append(f"  [missing_down_revision] {path.relative_to(REPO_ROOT)}")
    else:
        raw_down_revision = assignments.get("down_revision")
        try:
            down_revisions = parse_revision_refs(raw_down_revision, field_name="down_revision")
        except TypeError as exc:
            findings.append(f"  [invalid_down_revision] {path.relative_to(REPO_ROOT)} -> {exc}")

    depends_on: tuple[str, ...] = ()
    if "depends_on" in seen_fields:
        raw_depends_on = assignments.get("depends_on")
        try:
            depends_on = parse_revision_refs(raw_depends_on, field_name="depends_on")
        except TypeError as exc:
            findings.append(f"  [invalid_depends_on] {path.relative_to(REPO_ROOT)} -> {exc}")

    return MigrationInfo(
        path=path,
        revision=revision,
        down_revisions=down_revisions,
        depends_on=depends_on,
        findings=tuple(findings),
    )


def detect_cycle(revisions: dict[str, MigrationInfo]) -> tuple[str, ...] | None:
    visited: set[str] = set()
    visiting: set[str] = set()
    stack: list[str] = []

    def visit(revision: str) -> tuple[str, ...] | None:
        if revision in visiting:
            cycle_start = stack.index(revision)
            return tuple(stack[cycle_start:] + [revision])
        if revision in visited:
            return None

        visiting.add(revision)
        stack.append(revision)
        info = revisions[revision]
        for parent in info.down_revisions or ():
            if parent not in revisions:
                continue
            cycle = visit(parent)
            if cycle is not None:
                return cycle
        stack.pop()
        visiting.remove(revision)
        visited.add(revision)
        return None

    for revision in sorted(revisions):
        cycle = visit(revision)
        if cycle is not None:
            return cycle
    return None


def build_scope_report(root: Path) -> ScopeReport:
    findings: list[str] = []
    migrations = [parse_migration(path) for path in iter_version_files(root)]
    revision_map: dict[str, MigrationInfo] = {}
    referenced_revisions: set[str] = set()

    for info in migrations:
        findings.extend(info.findings)
        if info.revision is None:
            continue

        rel = info.path.relative_to(REPO_ROOT)
        if len(info.revision) > MAX_REVISION_LENGTH:
            findings.append(
                f"  [too_long] {rel} -> {info.revision} ({len(info.revision)} > {MAX_REVISION_LENGTH})"
            )

        previous = revision_map.get(info.revision)
        if previous is not None:
            findings.append(
                f"  [duplicate_revision] {info.revision} -> {previous.path.relative_to(REPO_ROOT)} | {rel}"
            )
            continue

        revision_map[info.revision] = info

    for revision, info in revision_map.items():
        rel = info.path.relative_to(REPO_ROOT)
        for parent in info.down_revisions or ():
            referenced_revisions.add(parent)
            if parent == revision:
                findings.append(f"  [self_reference] {rel} -> {revision}")
                continue
            if parent not in revision_map:
                findings.append(f"  [missing_parent] {rel} -> {parent}")

        for dependency in info.depends_on:
            if dependency == revision:
                findings.append(f"  [self_dependency] {rel} -> {revision}")
                continue
            if dependency not in revision_map:
                findings.append(f"  [missing_dependency] {rel} -> {dependency}")

    cycle = detect_cycle(revision_map)
    if cycle is not None:
        findings.append(f"  [cycle] {' -> '.join(cycle)}")

    heads = tuple(sorted(revision for revision in revision_map if revision not in referenced_revisions))
    if len(heads) == 0:
        findings.append("  [no_head] revision graph does not contain a head")
    elif len(heads) > 1:
        findings.append(f"  [multiple_heads] {', '.join(heads)}")

    return ScopeReport(findings=tuple(findings), heads=heads)


def main() -> int:
    findings_total = 0
    healthy_heads: list[str] = []

    for scope, root in VERSION_ROOTS.items():
        if not root.exists():
            continue

        report = build_scope_report(root)
        if report.findings:
            print()
            print(f"[{scope}] {root.relative_to(REPO_ROOT)}")
            for finding in report.findings:
                print(finding)
            findings_total += len(report.findings)
            continue

        head_text = report.heads[0] if report.heads else "<none>"
        healthy_heads.append(f"{scope}={head_text}")

    if findings_total == 0:
        suffix = f" Heads: {', '.join(healthy_heads)}." if healthy_heads else ""
        print(f"OK: Alembic revision graph is healthy.{suffix}")
        return 0

    print()
    print(f"FAIL: found {findings_total} Alembic revision issue(s).")
    print("Rules: revision id length <= 32, down_revision/depends_on must stay within the same backend, graph must be acyclic, and each backend must have exactly one head.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
