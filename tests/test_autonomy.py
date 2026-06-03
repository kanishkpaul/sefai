import random
from datetime import datetime, timedelta

import pytest

from backend.autonomy.engine import AutonomyEngine
from backend.memory.manager import MemoryManager
from backend.models import MessageRecord
from backend.persona.service import PersonaService
from backend.storage.database import Database


@pytest.mark.asyncio
async def test_autonomy_respects_disabled_state(tmp_path):
    db = Database(str(tmp_path / "companion.db"))
    await db.connect()
    persona = PersonaService("persona.sample.json").load()
    manager = MemoryManager(db)
    messages = []

    async def on_message(message: str) -> None:
        messages.append(message)

    engine = AutonomyEngine(db, persona, manager, on_message, random.Random(1))
    engine.set_autonomy_enabled(False)
    trigger = persona.message_triggers.time_based[0]
    decision = await engine.evaluate_trigger(trigger)
    assert decision.should_send is False
    await db.close()


@pytest.mark.asyncio
async def test_autonomy_queues_after_inactivity(tmp_path):
    db = Database(str(tmp_path / "companion.db"))
    await db.connect()
    persona = PersonaService("persona.sample.json").load()
    manager = MemoryManager(db)
    delivered = []

    async def on_message(message: str) -> None:
        delivered.append(message)

    await db.save_message(
        MessageRecord(
            role="user",
            content="Let's talk about intelligence.",
            initiated_by="user",
        )
    )
    await db.connection.execute(
        "UPDATE messages SET timestamp = ? WHERE id = 1",
        ((datetime.now() - timedelta(days=4)).isoformat(),),
    )
    await db.connection.commit()

    engine = AutonomyEngine(db, persona, manager, on_message, random.Random(1))
    engine.persona.quiet_hours.enabled = False
    messages = await engine.check_time_based_triggers()
    assert len(messages) == 1
    assert delivered
    await db.close()
