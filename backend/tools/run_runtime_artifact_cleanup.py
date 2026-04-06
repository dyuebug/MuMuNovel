from __future__ import annotations

import argparse
import json
from dataclasses import asdict, dataclass
from datetime import datetime, timedelta
from pathlib import Path


@dataclass
class MoveRecord:
    source: str
    destination: str


@dataclass
class DeleteRecord:
    path: str


def resolve_repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def month_bucket(path: Path) -> str:
    return datetime.fromtimestamp(path.stat().st_mtime).strftime('%Y-%m')


def move_file(path: Path, destination: Path, *, dry_run: bool, records: list[MoveRecord]) -> None:
    records.append(MoveRecord(source=str(path), destination=str(destination)))
    if dry_run:
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    path.replace(destination)


def delete_file(path: Path, *, dry_run: bool, records: list[DeleteRecord]) -> None:
    records.append(DeleteRecord(path=str(path)))
    if dry_run:
        return
    path.unlink(missing_ok=True)


def prune_empty_dirs(root: Path, *, dry_run: bool) -> None:
    if dry_run or not root.exists():
        return

    directories = sorted(
        (item for item in root.rglob('*') if item.is_dir()),
        key=lambda item: len(item.parts),
        reverse=True,
    )
    for directory in directories:
        try:
            next(directory.iterdir())
        except StopIteration:
            directory.rmdir()


def archive_live_artifacts(root: Path, *, keep_recent_rechecks: int, dry_run: bool) -> list[MoveRecord]:
    live_dir = root / 'tmp' / 'live'
    archive_dir = live_dir / 'archive'
    records: list[MoveRecord] = []

    if not live_dir.exists():
        return records

    rechecks = sorted(
        live_dir.glob('tmp_live_test_summary_recheck_*.json'),
        key=lambda item: item.stat().st_mtime,
        reverse=True,
    )
    for path in rechecks[keep_recent_rechecks:]:
        bucket = archive_dir / 'recheck' / month_bucket(path)
        move_file(path, bucket / path.name, dry_run=dry_run, records=records)

    for path in live_dir.glob('tmp_live_test_summary_*.json'):
        if path.name == 'tmp_live_test_summary_latest.json':
            continue
        if path.name.startswith('tmp_live_test_summary_recheck_'):
            continue
        bucket = archive_dir / 'summary' / month_bucket(path)
        move_file(path, bucket / path.name, dry_run=dry_run, records=records)

    return records


def archive_smoke_artifacts(root: Path, *, dry_run: bool) -> list[MoveRecord]:
    smoke_dir = root / 'tmp' / 'smoke'
    archive_dir = smoke_dir / 'archive'
    records: list[MoveRecord] = []

    if not smoke_dir.exists():
        return records

    for path in smoke_dir.glob('tmp_live_batch_smoke_[0-9]*.json'):
        bucket = archive_dir / 'live-batch' / month_bucket(path)
        move_file(path, bucket / path.name, dry_run=dry_run, records=records)

    for path in smoke_dir.glob('tmp_settings_probe_smoke*.json'):
        if path.name == 'tmp_settings_probe_smoke_latest.json':
            continue
        bucket = archive_dir / 'settings-probe' / month_bucket(path)
        move_file(path, bucket / path.name, dry_run=dry_run, records=records)

    return records


def is_older_than(path: Path, *, cutoff: datetime) -> bool:
    return datetime.fromtimestamp(path.stat().st_mtime) < cutoff


def delete_archive_files(archive_dir: Path, *, cutoff: datetime, dry_run: bool) -> list[DeleteRecord]:
    records: list[DeleteRecord] = []
    if not archive_dir.exists():
        return records

    files = sorted(
        (item for item in archive_dir.rglob('*') if item.is_file()),
        key=lambda item: item.stat().st_mtime,
    )
    for path in files:
        if is_older_than(path, cutoff=cutoff):
            delete_file(path, dry_run=dry_run, records=records)

    prune_empty_dirs(archive_dir, dry_run=dry_run)
    return records


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description='Archive runtime artifacts under tmp/.')
    parser.add_argument('--root', type=Path, default=resolve_repo_root(), help='Repository root path')
    parser.add_argument('--keep-live-rechecks', type=int, default=3, help='How many recent live recheck files to keep in tmp/live')
    parser.add_argument('--delete-older-than-days', type=int, default=None, help='Delete archived files older than the given number of days')
    parser.add_argument('--delete-live-archive', action='store_true', help='Only delete expired files under tmp/live/archive')
    parser.add_argument('--delete-smoke-archive', action='store_true', help='Only delete expired files under tmp/smoke/archive')
    parser.add_argument('--dry-run', action='store_true', help='Preview planned moves or deletions without changing files')
    return parser.parse_args()


def resolve_delete_targets(root: Path, args: argparse.Namespace) -> list[Path]:
    if args.delete_older_than_days is None:
        return []

    targets: list[Path] = []
    if args.delete_live_archive:
        targets.append(root / 'tmp' / 'live' / 'archive')
    if args.delete_smoke_archive:
        targets.append(root / 'tmp' / 'smoke' / 'archive')
    if targets:
        return targets

    return [
        root / 'tmp' / 'live' / 'archive',
        root / 'tmp' / 'smoke' / 'archive',
    ]


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    keep_live_rechecks = max(args.keep_live_rechecks, 0)

    live_records = archive_live_artifacts(root, keep_recent_rechecks=keep_live_rechecks, dry_run=args.dry_run)
    smoke_records = archive_smoke_artifacts(root, dry_run=args.dry_run)

    deleted_live_records: list[DeleteRecord] = []
    deleted_smoke_records: list[DeleteRecord] = []
    if args.delete_older_than_days is not None:
        cutoff = datetime.now() - timedelta(days=max(args.delete_older_than_days, 0))
        for target in resolve_delete_targets(root, args):
            deleted_records = delete_archive_files(target, cutoff=cutoff, dry_run=args.dry_run)
            target_string = target.as_posix().lower()
            if target_string.endswith('/tmp/live/archive'):
                deleted_live_records.extend(deleted_records)
            elif target_string.endswith('/tmp/smoke/archive'):
                deleted_smoke_records.extend(deleted_records)

    summary = {
        'root': str(root),
        'dry_run': bool(args.dry_run),
        'keep_live_rechecks': keep_live_rechecks,
        'delete_older_than_days': args.delete_older_than_days,
        'moved': {
            'live': [asdict(item) for item in live_records],
            'smoke': [asdict(item) for item in smoke_records],
        },
        'deleted': {
            'live': [asdict(item) for item in deleted_live_records],
            'smoke': [asdict(item) for item in deleted_smoke_records],
        },
        'counts': {
            'moved_live': len(live_records),
            'moved_smoke': len(smoke_records),
            'moved_total': len(live_records) + len(smoke_records),
            'deleted_live': len(deleted_live_records),
            'deleted_smoke': len(deleted_smoke_records),
            'deleted_total': len(deleted_live_records) + len(deleted_smoke_records),
        },
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
