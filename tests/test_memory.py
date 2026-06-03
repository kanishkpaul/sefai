import pytest

from backend.config import RuntimeSettings
from backend.memory.manager import MemoryManager
from backend.models import MessageRecord
from backend.persona.service import PersonaService
from backend.storage.database import Database


@pytest.mark.asyncio
async def test_memory_context_includes_history_and_summary(tmp_path):
    db = Database(str(tmp_path / "companion.db"))
    await db.connect()
    await db.save_message(MessageRecord(role="user", content="I keep thinking about intelligence and truth."))
    await db.save_message(MessageRecord(role="companion", content="Then define what you mean by intelligence."))
    await db.save_memory_summary("They are preoccupied with intelligence.", "relationship", 0.8)
    persona = PersonaService("persona.sample.json").load()
    manager = MemoryManager(db)
    context = await manager.build_context("What do you think now?", persona.relationship_state.current_understanding)
    assert "Recent conversation:" in context
    assert "Key memories:" in context
    assert len(context) < 6001
    await db.close()
