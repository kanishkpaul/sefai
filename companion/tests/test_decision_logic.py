import pytest

from backend.app import CompanionApplication
from backend.config import RuntimeSettings
from backend.models import MessageEnvelope


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


@pytest.mark.asyncio
async def test_greeting_is_not_refused(tmp_path):
    settings = RuntimeSettings(
        persona_path="persona.sample.json",
        database_path=str(tmp_path / "companion.db"),
        autonomy_enabled=False,
    )
    app = CompanionApplication(settings)
    await app.db.connect()
    app.persona = app.persona_service.load()

    first = app._evaluate_message_against_persona("hi")
    second = app._evaluate_message_against_persona("what's your name?")

    assert first.should_refuse is False
    assert second.should_refuse is False
    await app.db.close()


@pytest.mark.asyncio
async def test_surface_refusal_requires_withdrawn_state(tmp_path):
    settings = RuntimeSettings(
        persona_path="persona.sample.json",
        database_path=str(tmp_path / "companion.db"),
        autonomy_enabled=False,
    )
    app = CompanionApplication(settings)
    await app.db.connect()
    app.persona = app.persona_service.load()

    first = app._evaluate_message_against_persona("ok")
    second = app._evaluate_message_against_persona("whatever")

    assert first.should_refuse is False
    assert second.should_refuse is True
    assert second.reason == "surface_level_when_withdrawn"
    await app.db.close()


@pytest.mark.asyncio
async def test_connection_prompt_variants_are_treated_as_connection(tmp_path):
    settings = RuntimeSettings(
        persona_path="persona.sample.json",
        database_path=str(tmp_path / "companion.db"),
        autonomy_enabled=False,
    )
    app = CompanionApplication(settings)
    await app.db.connect()
    app.persona = app.persona_service.load()

    assert app._is_simple_connection_prompt("hiii") is True
    assert app._is_simple_connection_prompt("can you talk to me") is True
    assert app._generate_connection_reply("what's your name?").startswith("I'm Aria")
    await app.db.close()


@pytest.mark.asyncio
async def test_pause_and_resume_autonomy_persist_setting(tmp_path):
    settings = RuntimeSettings(
        persona_path="persona.sample.json",
        database_path=str(tmp_path / "companion.db"),
        autonomy_enabled=True,
    )
    app = CompanionApplication(settings)
    await app.db.connect()
    app.persona = app.persona_service.load()

    pause_response = await app._handle_pause_autonomy(MessageEnvelope(type="pause_autonomy", request_id="pause"))
    persisted_after_pause = await app.db.fetch_settings()

    resume_response = await app._handle_resume_autonomy(MessageEnvelope(type="resume_autonomy", request_id="resume"))
    persisted_after_resume = await app.db.fetch_settings()

    assert pause_response.payload["autonomy_enabled"] is False
    assert persisted_after_pause["autonomy_enabled"] is False
    assert resume_response.payload["autonomy_enabled"] is True
    assert persisted_after_resume["autonomy_enabled"] is True
    await app.db.close()
