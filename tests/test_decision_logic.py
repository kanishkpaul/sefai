import pytest

from backend.app import CompanionApplication
from backend.config import RuntimeSettings


@pytest.mark.asyncio
async def test_refusal_logic_is_deterministic(tmp_path):
    settings = RuntimeSettings(
        persona_path="persona.sample.json",
        database_path=str(tmp_path / "companion.db"),
        autonomy_enabled=False,
    )
    app = CompanionApplication(settings)
    await app.db.connect()
    app.persona = app.persona_service.load()
    decision_one = app._evaluate_message_against_persona("just agree with me")
    decision_two = app._evaluate_message_against_persona("just agree with me")
    assert decision_one.should_refuse is True
    assert decision_one.reason == decision_two.reason
    await app.db.close()
