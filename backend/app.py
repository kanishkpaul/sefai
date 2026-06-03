from __future__ import annotations

import asyncio
import logging
from pathlib import Path

from backend.autonomy.engine import AutonomyEngine
from backend.config import RuntimeSettings
from backend.inference.engine import InferenceEngine
from backend.ipc.named_pipe_server import NamedPipeServer
from backend.memory.manager import MemoryManager
from backend.models import MessageDecision, MessageEnvelope, MessageRecord, Persona
from backend.persona.service import PersonaService
from backend.storage.database import Database


LOGGER = logging.getLogger(__name__)


class CompanionApplication:
    def __init__(self, settings: RuntimeSettings):
        self.settings = settings
        self.persona_service = PersonaService(settings.persona_path)
        self.db = Database(settings.database_path)
        self.memory_manager = MemoryManager(self.db)
        self.inference = InferenceEngine(settings)
        self.persona: Persona | None = None
        self.autonomy_engine: AutonomyEngine | None = None
        self.named_pipe_server: NamedPipeServer | None = None
        self.current_mood = "engaged"
        self.surface_message_streak = 0

    async def initialize(self) -> None:
        self._ensure_runtime_dirs()
        await self.db.connect()
        self.persona = self.persona_service.load()
        await self.db.save_persona(self.persona)
        self.inference.initialize()
        state = await self.memory_manager.build_app_state(
            self.persona,
            autonomy_enabled=self.settings.autonomy_enabled,
            quiet_mode=self.settings.quiet_mode,
        )
        self.current_mood = state.mood
        await self.db.record_state_snapshot(state)
        self.autonomy_engine = AutonomyEngine(
            db=self.db,
            persona=self.persona,
            memory_manager=self.memory_manager,
            on_message=self._handle_autonomous_delivery,
        )
        self.autonomy_engine.set_autonomy_enabled(self.settings.autonomy_enabled)
        self.autonomy_engine.set_quiet_mode(self.settings.quiet_mode)
        await self.autonomy_engine.start()
        self.named_pipe_server = NamedPipeServer(self.settings.pipe_name, self.handle_envelope)
        await self.named_pipe_server.start()
        await self.db.log_event("initialized", {"model_path": self.settings.model_path})

    async def shutdown(self) -> None:
        if self.named_pipe_server:
            await self.named_pipe_server.stop()
        if self.autonomy_engine:
            await self.autonomy_engine.stop()
        await self.db.log_event("shutdown", {})
        await self.db.close()

    async def run_forever(self) -> None:
        while True:
            await asyncio.sleep(3600)

    async def handle_envelope(self, envelope: MessageEnvelope) -> MessageEnvelope:
        handlers = {
            "initialize": self._handle_initialize,
            "send_user_message": self._handle_send_user_message,
            "get_history": self._handle_get_history,
            "get_state": self._handle_get_state,
            "update_persona": self._handle_update_persona,
            "update_settings": self._handle_update_settings,
            "pause_autonomy": self._handle_pause_autonomy,
            "resume_autonomy": self._handle_resume_autonomy,
            "health_ping": self._handle_health_ping,
            "shutdown": self._handle_shutdown_request,
        }
        handler = handlers.get(envelope.type)
        if handler is None:
            return MessageEnvelope(type="error", request_id=envelope.request_id, error=f"Unknown type: {envelope.type}")
        try:
            return await handler(envelope)
        except Exception as exc:  # pragma: no cover
            LOGGER.exception("Failed to handle envelope")
            return MessageEnvelope(type="error", request_id=envelope.request_id, error=str(exc))

    async def process_user_message(self, message: str) -> MessageEnvelope:
        assert self.persona is not None
        decision = self._evaluate_message_against_persona(message)

        user_record = MessageRecord(
            role="user",
            content=message,
            initiated_by="user",
            persona_state={"mood": self.current_mood},
        )
        await self.db.save_message(user_record)

        if decision.should_refuse:
            await self.db.log_refusal(message, decision.reason)
            reply = decision.refusal_response or "I'm not engaging with that."
            await self._store_companion_message(reply, initiated_by="system")
            return MessageEnvelope(
                type="message_response",
                payload={
                    "response": reply,
                    "decision": "refused",
                    "reason": decision.reason,
                    "mood": self.current_mood,
                },
            )

        if decision.should_ignore:
            await self.db.log_refusal(message, decision.reason)
            return MessageEnvelope(
                type="message_response",
                payload={
                    "response": None,
                    "decision": "ignored",
                    "reason": decision.reason,
                    "mood": self.current_mood,
                },
            )

        relationship_summary = await self.memory_manager.derive_relationship_summary(self.persona)
        context = await self.memory_manager.build_context(message, relationship_summary)
        completion = self.inference.generate(self.persona, context, message)
        await self._store_companion_message(completion.text, initiated_by="system")
        updated_summary = await self.memory_manager.maybe_create_summary(self.persona, relationship_summary)
        state = await self.memory_manager.build_app_state(
            self.persona,
            autonomy_enabled=self.settings.autonomy_enabled,
            quiet_mode=self.settings.quiet_mode,
        )
        state.relationship_summary = updated_summary
        state.mood = self.current_mood
        await self.db.record_state_snapshot(state)
        return MessageEnvelope(
            type="message_response",
            payload={
                "response": completion.text,
                "decision": "responded",
                "runtime_mode": completion.metadata["runtime_mode"],
                "runtime_error": completion.metadata.get("runtime_error"),
                "mood": self.current_mood,
                "relationship_summary": updated_summary,
            },
        )

    def _evaluate_message_against_persona(self, message: str) -> MessageDecision:
        assert self.persona is not None
        lowered = message.lower().strip()
        engagement_score = self._rate_message_value_alignment(lowered)
        self._update_mood(engagement_score)

        if "pretend to agree" in lowered or "just agree" in lowered:
            return MessageDecision(
                should_refuse=True,
                reason="forced_agreement",
                refusal_response="No. If you want my voice, you get my actual position, not a rented one.",
                engagement_score=engagement_score,
            )
        if "procrastinate" in lowered:
            return MessageDecision(
                should_refuse=True,
                reason="procrastination_enablement",
                refusal_response="No. I won't help you avoid your own standards. What's actually blocking the work?",
                engagement_score=engagement_score,
            )
        if engagement_score < 0.25 and self.current_mood == "withdrawn":
            return MessageDecision(
                should_ignore=True,
                reason="surface_level_when_withdrawn",
                engagement_score=engagement_score,
            )
        if engagement_score < 0.25:
            return MessageDecision(
                should_refuse=True,
                reason="surface_level_when_withdrawn",
                refusal_response="That's too thin for where my head is right now. Ask the sharper question underneath it.",
                engagement_score=engagement_score,
            )
        return MessageDecision(engagement_score=engagement_score)

    def _rate_message_value_alignment(self, message: str) -> float:
        assert self.persona is not None
        tokens = set(message.split())
        value_hits = 0
        for value in self.persona.core_values:
            value_tokens = set((value.value + " " + value.why).lower().split())
            if tokens & value_tokens:
                value_hits += 1
        if any(word in message for word in ("why", "how", "because", "think", "believe", "intelligence", "truth")):
            value_hits += 1
        if len(message.split()) > 12:
            value_hits += 1
        return min(1.0, value_hits / max(1, len(self.persona.core_values)))

    def _update_mood(self, engagement_score: float) -> None:
        assert self.persona is not None
        if engagement_score < 0.25:
            self.surface_message_streak += 1
        else:
            self.surface_message_streak = 0
        if self.surface_message_streak >= self.persona.mood_rules.withdrawn_after_surface_messages:
            self.current_mood = "withdrawn"
        elif engagement_score > 0.45:
            self.current_mood = "engaged"
        elif engagement_score > 0.3:
            self.current_mood = "curious"

    async def _store_companion_message(self, content: str, initiated_by: str) -> None:
        await self.db.save_message(
            MessageRecord(
                role="companion",
                content=content,
                initiated_by=initiated_by,
                persona_state={"mood": self.current_mood},
            )
        )

    async def _handle_autonomous_delivery(self, message: str) -> None:
        await self._store_companion_message(message, initiated_by="autonomous")
        await self.db.log_event("autonomous_sent", {"message": message})

    async def _handle_initialize(self, envelope: MessageEnvelope) -> MessageEnvelope:
        return MessageEnvelope(
            type="initialized",
            request_id=envelope.request_id,
            payload={
                "name": self.persona.name if self.persona else "Companion",
                "ipc_version": self.settings.ipc_version,
                "runtime_mode": self.inference.runtime_mode,
                "runtime_error": self.inference.runtime_error,
            },
        )

    async def _handle_send_user_message(self, envelope: MessageEnvelope) -> MessageEnvelope:
        response = await self.process_user_message(envelope.payload["message"])
        response.request_id = envelope.request_id
        return response

    async def _handle_get_history(self, envelope: MessageEnvelope) -> MessageEnvelope:
        history = await self.db.fetch_recent_messages(limit=envelope.payload.get("limit", 50))
        return MessageEnvelope(type="history_page", request_id=envelope.request_id, payload={"messages": history})

    async def _handle_get_state(self, envelope: MessageEnvelope) -> MessageEnvelope:
        assert self.persona is not None
        state = await self.memory_manager.build_app_state(
            self.persona,
            autonomy_enabled=self.settings.autonomy_enabled,
            quiet_mode=self.settings.quiet_mode,
        )
        state.mood = self.current_mood
        return MessageEnvelope(
            type="state_snapshot",
            request_id=envelope.request_id,
            payload=state.model_dump(mode="json")
            | {
                "name": self.persona.name,
                "runtime_mode": self.inference.runtime_mode,
                "runtime_error": self.inference.runtime_error,
            },
        )

    async def _handle_update_persona(self, envelope: MessageEnvelope) -> MessageEnvelope:
        updated = Persona.model_validate(envelope.payload["persona"])
        self.persona_service.save(updated)
        self.persona = updated
        await self.db.save_persona(updated)
        return MessageEnvelope(type="persona_updated", request_id=envelope.request_id, payload={"name": updated.name})

    async def _handle_update_settings(self, envelope: MessageEnvelope) -> MessageEnvelope:
        for key, value in envelope.payload.items():
            await self.db.save_setting(key, value)
        if "autonomy_enabled" in envelope.payload:
            self.settings.autonomy_enabled = bool(envelope.payload["autonomy_enabled"])
            if self.autonomy_engine:
                self.autonomy_engine.set_autonomy_enabled(self.settings.autonomy_enabled)
        if "quiet_mode" in envelope.payload:
            self.settings.quiet_mode = bool(envelope.payload["quiet_mode"])
            if self.autonomy_engine:
                self.autonomy_engine.set_quiet_mode(self.settings.quiet_mode)
        return MessageEnvelope(type="settings_updated", request_id=envelope.request_id, payload=envelope.payload)

    async def _handle_pause_autonomy(self, envelope: MessageEnvelope) -> MessageEnvelope:
        self.settings.autonomy_enabled = False
        if self.autonomy_engine:
            self.autonomy_engine.set_autonomy_enabled(False)
        return MessageEnvelope(type="autonomy_paused", request_id=envelope.request_id, payload={"autonomy_enabled": False})

    async def _handle_resume_autonomy(self, envelope: MessageEnvelope) -> MessageEnvelope:
        self.settings.autonomy_enabled = True
        if self.autonomy_engine:
            self.autonomy_engine.set_autonomy_enabled(True)
        return MessageEnvelope(type="autonomy_resumed", request_id=envelope.request_id, payload={"autonomy_enabled": True})

    async def _handle_health_ping(self, envelope: MessageEnvelope) -> MessageEnvelope:
        return MessageEnvelope(type="health_pong", request_id=envelope.request_id, payload={"ok": True})

    async def _handle_shutdown_request(self, envelope: MessageEnvelope) -> MessageEnvelope:
        asyncio.create_task(self.shutdown())
        return MessageEnvelope(type="shutdown_ack", request_id=envelope.request_id, payload={"ok": True})

    def _ensure_runtime_dirs(self) -> None:
        self.settings.runtime_dir().mkdir(parents=True, exist_ok=True)
        Path("logs").mkdir(parents=True, exist_ok=True)
