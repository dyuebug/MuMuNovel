from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict


@dataclass(frozen=True)
class BatchGenerationTaskViewContext:
    task: Any
    quality_snapshot: Dict[str, Any]
    workflow_snapshot: Dict[str, Any]
