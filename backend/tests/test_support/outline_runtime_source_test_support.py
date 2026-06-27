from __future__ import annotations

import json
from typing import Any, Dict, Optional


def build_outline_structure_runtime_sources(outline: Optional[Any]) -> Dict[str, Any]:
    structure_text = getattr(outline, "structure", None)
    if not isinstance(structure_text, str) or not structure_text.strip():
        return {}

    try:
        structure = json.loads(structure_text)
    except (TypeError, ValueError, json.JSONDecodeError):
        return {}

    if not isinstance(structure, dict):
        return {}

    character_focus: list[str] = []
    character_state_ledger: list[str] = []
    organization_state_ledger: list[str] = []
    for item in structure.get("characters") or []:
        if not isinstance(item, dict):
            continue
        name = str(item.get("name") or "").strip()
        item_type = str(item.get("type") or "character").strip().lower()
        if not name:
            continue
        if item_type == "organization":
            if name not in organization_state_ledger:
                organization_state_ledger.append(f"{name}: active in this chapter outline")
            continue
        if name not in character_focus:
            character_focus.append(name)
        entry = f"{name}: active in this chapter outline"
        if entry not in character_state_ledger:
            character_state_ledger.append(entry)

    runtime_sources: Dict[str, Any] = {}
    if character_focus:
        runtime_sources["character_focus"] = character_focus[:4]
    if character_state_ledger:
        runtime_sources["character_state_ledger"] = character_state_ledger[:4]
    if organization_state_ledger:
        runtime_sources["organization_state_ledger"] = organization_state_ledger[:4]
    return runtime_sources
