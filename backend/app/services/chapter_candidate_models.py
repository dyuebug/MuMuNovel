from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List


@dataclass(slots=True)
class ChapterCandidateWorkingSet:
    selected_candidate: Dict[str, Any]
    candidates: List[Dict[str, Any]]

    @property
    def candidate_count(self) -> int:
        return len(self.candidates)
