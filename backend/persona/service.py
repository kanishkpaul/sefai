from __future__ import annotations

import json
from pathlib import Path

from backend.models import Persona


class PersonaService:
    def __init__(self, persona_path: str):
        self.persona_path = Path(persona_path)

    def load(self) -> Persona:
        with self.persona_path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
        return Persona.model_validate(payload)

    def save(self, persona: Persona) -> None:
        self.persona_path.parent.mkdir(parents=True, exist_ok=True)
        with self.persona_path.open("w", encoding="utf-8") as handle:
            json.dump(persona.model_dump(mode="json"), handle, indent=2)
