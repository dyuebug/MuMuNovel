# 13 - Project Structure Governance

This document defines the ownership and storage rules for runtime artifacts,
logs, scripts, and formal documentation in the MuMuNovel repository.

## Goals

- Keep runtime outputs out of the repository root.
- Separate temporary artifacts from formal documentation.
- Standardize how `backend/tools/` writes to `tmp/`, `logs/`, and `docs/`.
- Make cleanup, release, and regression scripts easier to maintain.

## Root Layout

```text
MuMuNovel/
- backend/               # backend services, APIs, scripts, tests
- frontend/              # frontend application
- docs/                  # formal documentation
- images/                # documentation images and assets
- logs/                  # runtime and ops logs
- tmp/                   # temporary artifacts, caches, test outputs
- data/                  # local/runtime data
- profiling-artifacts/   # profiling outputs
- Dockerfile
- docker-compose.yml
- redeploy.ps1
- redeploy.bat
- release.ps1
- release.bat
- README.md
```

The repository root should not keep scattered temporary files such as:

- `tmp_*.json` or `tmp_*.log`
- one-off cache files
- manually exported investigation outputs

## Governance Rules

### 1. `backend/tools/`

All maintenance, regression, smoke, and cleanup scripts should live in
`backend/tools/`.

Examples:

- `backend/tools/run_live_regression_retest.py`
- `backend/tools/run_settings_probe_smoke.py`
- `backend/tools/run_batch_terminal_status_smoke.py`
- `backend/tools/run_runtime_artifact_cleanup.py`

### 2. `tmp/`

`tmp/` stores temporary runtime artifacts and should not be used for formal
documentation.

```text
tmp/
- README.md
- cache/                          # rebuildable caches
- live/                           # live regression outputs
  - tmp_live_test_summary_latest.json
  - tmp_live_test_summary_recheck_*.json
  - archive/
    - recheck/YYYY-MM/
    - summary/YYYY-MM/
- smoke/                          # smoke outputs
  - tmp_live_batch_smoke_latest.json
  - tmp_settings_probe_smoke_latest.json
  - archive/
    - live-batch/YYYY-MM/
    - settings-probe/YYYY-MM/
- provider/                       # provider diagnostics
- misc/                           # other temporary outputs
```

### 3. `logs/`

`logs/` stores runtime and operational logs only.

```text
logs/
- README.md
- app.log
- dev/                            # local development logs
- ops/                            # redeploy / release logs
- ab_chapter_rules/
```

Conventions:

- `redeploy.ps1` writes to `logs/ops/redeploy.log`
- `release.ps1` writes to `logs/ops/release.log`

### 4. `docs/architecture/`

Formal architecture documents should be stored in `docs/architecture/`, not in
`tmp/` or `logs/`.

Example:

- `docs/architecture/chapter-generation-recovery.md`

## Retention and Archiving

### Live regression

- Keep the latest outputs in `tmp/live/`
- Archive older `recheck` outputs in `tmp/live/archive/recheck/YYYY-MM/`
- Archive older latest/summary outputs in `tmp/live/archive/summary/YYYY-MM/`

### Smoke outputs

- Keep only the latest smoke outputs in `tmp/smoke/`
- Archive historical live-batch and settings-probe outputs in the matching
  `archive/YYYY-MM/` directories

### Cleanup policy

- Temporary artifacts must be safe to delete and regenerate.
- Use `backend/tools/run_runtime_artifact_cleanup.py` for regular cleanup.
- Prefer `--dry-run` before destructive cleanup.

## Recommended Commands

### Preview cleanup

```bash
python -X utf8 backend/tools/run_runtime_artifact_cleanup.py --dry-run
```

### Execute cleanup

```bash
python -X utf8 backend/tools/run_runtime_artifact_cleanup.py
```

### Delete archives older than 30 days

```bash
python -X utf8 backend/tools/run_runtime_artifact_cleanup.py --dry-run --delete-older-than-days 30
```

### Preview cleanup including live archive deletion

```bash
python -X utf8 backend/tools/run_runtime_artifact_cleanup.py --dry-run --delete-older-than-days 30 --delete-live-archive
```

## Notes

- New scripts should reuse the existing `backend/tools/` conventions.
- Runtime outputs should go into `tmp/` or `logs/`, not the repository root.
- Formal documentation should stay in `docs/`.
- Use dry-run before deletion whenever possible.
