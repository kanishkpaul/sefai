from __future__ import annotations

import asyncio
import json
import random
from dataclasses import dataclass
from datetime import datetime, time, timedelta
from typing import Awaitable, Callable

from apscheduler.schedulers.asyncio import AsyncIOScheduler

from backend.memory.manager import MemoryManager
from backend.models import AppState, Persona, TriggerRule
from backend.storage.database import Database


AutonomousCallback = Callable[[str], Awaitable[None]]


@dataclass(slots=True)
class AutonomyDecision:
    should_send: bool
    reason: str


class AutonomyEngine:
    def __init__(
        self,
        db: Database,
        persona: Persona,
        memory_manager: MemoryManager,
        on_message: AutonomousCallback,
        random_source: random.Random | None = None,
    ):
        self.db = db
        self.persona = persona
        self.memory_manager = memory_manager
        self.on_message = on_message
        self.random = random_source or random.Random()
        self.scheduler = AsyncIOScheduler()
        self.autonomy_enabled = True
        self.quiet_mode = False

    async def start(self) -> None:
        self.scheduler.add_job(self._spawn_time_check, "cron", hour=12, minute=0)
        self.scheduler.add_job(self._spawn_state_check, "interval", hours=6)
        self.scheduler.start()

    async def stop(self) -> None:
        if self.scheduler.running:
            self.scheduler.shutdown(wait=False)

    def _spawn_time_check(self) -> None:
        asyncio.create_task(self.check_time_based_triggers())

    def _spawn_state_check(self) -> None:
        asyncio.create_task(self.check_state_based_triggers())

    def set_autonomy_enabled(self, enabled: bool) -> None:
        self.autonomy_enabled = enabled

    def set_quiet_mode(self, quiet_mode: bool) -> None:
        self.quiet_mode = quiet_mode

    async def check_time_based_triggers(self) -> list[str]:
        messages = []
        for trigger in self.persona.message_triggers.time_based:
            decision = await self.evaluate_trigger(trigger)
            if decision.should_send:
                messages.append(await self.queue_trigger_message(trigger, decision.reason))
        return messages

    async def check_state_based_triggers(self) -> list[str]:
        if not self.autonomy_enabled or self.quiet_mode:
            return []
        recent = await self.db.fetch_recent_messages(limit=5)
        messages = []
        for message in reversed(recent):
            if message["role"] != "user":
                continue
            lowered = message["content"].lower()
            if "whatever" in lowered or "just agree" in lowered:
                trigger = TriggerRule(
                    name="Value conflict follow-up",
                    condition="recent_message_conflicts_with_values",
                    action="Ask them to be more precise.",
                    probability=0.55,
                )
                messages.append(await self.queue_trigger_message(trigger, "state_conflict"))
                break
        return messages

    async def evaluate_trigger(self, trigger: TriggerRule) -> AutonomyDecision:
        if not self.autonomy_enabled:
            return AutonomyDecision(False, "autonomy_disabled")
        if self.quiet_mode:
            return AutonomyDecision(False, "quiet_mode")
        if self._is_quiet_hours():
            return AutonomyDecision(False, "quiet_hours")

        if trigger.condition == "no_user_message_for_days":
            last_user_message_at = await self.db.get_last_user_message_at()
            if last_user_message_at is None:
                return AutonomyDecision(False, "no_conversation_yet")
            days = trigger.days or 3
            if datetime.now() - last_user_message_at < timedelta(days=days):
                return AutonomyDecision(False, "cooldown_not_reached")

        probability = trigger.probability if trigger.probability is not None else 0.7
        if self.random.random() > probability:
            return AutonomyDecision(False, "probability_skip")
        return AutonomyDecision(True, "trigger_fired")

    async def queue_trigger_message(self, trigger: TriggerRule, reason: str) -> str:
        message = await self.generate_autonomous_message(trigger)
        scheduled_time = datetime.now() + timedelta(minutes=1)
        queue_id = await self.db.enqueue_autonomous_message(
            trigger_type=trigger.name,
            trigger_condition=json.dumps(trigger.model_dump(mode="json")),
            scheduled_time=scheduled_time,
            delivered_message=message,
        )
        await self.db.log_event("autonomy_queued", {"queue_id": queue_id, "reason": reason, "message": message})
        await self.on_message(message)
        await self.db.mark_autonomous_message_sent(queue_id)
        return message

    async def generate_autonomous_message(self, trigger: TriggerRule) -> str:
        state = await self.memory_manager.build_app_state(
            self.persona,
            autonomy_enabled=self.autonomy_enabled,
            quiet_mode=self.quiet_mode,
        )
        first_goal = self.persona.autonomy_framework.independent_goals[0].goal
        if trigger.example:
            return trigger.example
        return (
            f"I've been thinking about us. In a {state.mood} state, I keep returning to {first_goal.lower()}. "
            "Answer me honestly: what are you avoiding saying out loud?"
        )

    def _is_quiet_hours(self) -> bool:
        if not self.persona.quiet_hours.enabled:
            return False
        now = datetime.now().time()
        start = time.fromisoformat(self.persona.quiet_hours.start)
        end = time.fromisoformat(self.persona.quiet_hours.end)
        if start <= end:
            return start <= now <= end
        return now >= start or now <= end
