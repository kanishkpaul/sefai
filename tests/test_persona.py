from pathlib import Path

import pytest
from pydantic import ValidationError

from backend.models import Persona
from backend.persona.service import PersonaService


def test_persona_validation_accepts_sample():
    persona = PersonaService("persona.sample.json").load()
    assert persona.name == "Aria"
    assert persona.mood_rules.default_mood == "engaged"


def test_persona_validation_rejects_invalid_shape(tmp_path: Path):
    broken = tmp_path / "persona.json"
    broken.write_text('{"name":"Aria"}', encoding="utf-8")
    with pytest.raises(ValidationError):
        PersonaService(str(broken)).load()
