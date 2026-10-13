from __future__ import annotations

from collections import Counter
from typing import Iterable

from backend.models import AppState, Persona
from backend.storage.database import Database


def _tokenize(text: str) -> list[str]:
    return [token.strip(".,!?;:()[]{}\"'").lower() for token in text.split() if token.strip()]


class MemoryManager:
    def __init__(self, db: Database):
        self.db = db

    async def build_context(self, user_message: str, relationship_summary: str, max_chars: int = 6000) -> str:
        recent = await self.db.fetch_recent_messages(limit=20)
        memories = await self.db.fetch_memory_summaries(limit=5)
        lines = ["Recent conversation:"]
        for item in recent:
            lines.append(f"{item['role']}: {item['content']}")
        lines.append("")
        lines.append("Key memories:")
        for memory in memories:
            lines.append(f"- {memory['summary']}")
        lines.append("")
        lines.append("Current relationship summary:")
        lines.append(relationship_summary)
        lines.append("")
        lines.append("Current user message:")
        lines.append(user_message)
        context = "\n".join(lines)
        return context[:max_chars]

    async def maybe_create_summary(self, persona: Persona, relationship_summary: str) -> str:
        messages = await self.db.fetch_recent_messages(limit=6)
        if len(messages) < 4:
            return relationship_summary

        recent_text = " ".join(message["content"] for message in messages)
        keywords = self._top_keywords(recent_text, persona)
        if not keywords:
            return relationship_summary

        summary = (
            f"They have been focused on {', '.join(keywords[:3])}. "
            f"This reinforces that {relationship_summary.lower()}"
        )
        await self.db.save_memory_summary(summary, relevant_to="relationship_state", importance_score=0.55)
        return summary

    async def derive_relationship_summary(self, persona: Persona) -> str:
        recent = await self.db.fetch_recent_messages(limit=10)
        if not recent:
            return persona.relationship_state.current_understanding

        user_only = " ".join(item["content"] for item in recent if item["role"] == "user")
        keywords = self._top_keywords(user_only, persona)
        if not keywords:
            return persona.relationship_state.current_understanding

        return (
            f"They keep returning to {', '.join(keywords[:3])}. "
            f"I read that as part of how they make meaning."
        )

    def _top_keywords(self, text: str, persona: Persona) -> list[str]:
        stop_words = {
            "the", "a", "and", "or", "to", "of", "is", "it", "that", "i", "you",
            "they", "we", "in", "on", "for", "with", "this", "be", "are", "was",
            "have", "has", "had", "do", "does", "did", "not", "but", "if", "so",
        }
        persona_tokens = set()
        for value in persona.core_values:
            persona_tokens.update(_tokenize(value.value))
            persona_tokens.update(_tokenize(value.why))
        counts = Counter(token for token in _tokenize(text) if token not in stop_words)
        ranked = sorted(counts.items(), key=lambda item: (-item[1], item[0]))
        boosted = [token for token, _ in ranked if token in persona_tokens]
        remaining = [token for token, _ in ranked if token not in persona_tokens]
        return boosted + remaining

    async def build_app_state(self, persona: Persona, autonomy_enabled: bool, quiet_mode: bool) -> AppState:
        latest = await self.db.fetch_latest_state()
        relationship_summary = await self.derive_relationship_summary(persona)
        mood = latest.mood if latest else persona.mood_rules.default_mood
        active_goals = [goal.goal for goal in persona.autonomy_framework.independent_goals[:3]]
        return AppState(
            mood=mood,
            relationship_summary=relationship_summary,
            active_goals=active_goals,
            autonomy_enabled=autonomy_enabled,
            quiet_mode=quiet_mode,
            last_user_message_at=await self.db.get_last_user_message_at(),
        )
