from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from backend.config import RuntimeSettings
from backend.models import Persona

try:
    from llama_cpp import Llama  # type: ignore
except Exception:  # pragma: no cover
    Llama = None


@dataclass(slots=True)
class CompletionResult:
    text: str
    metadata: dict[str, Any]


class InferenceEngine:
    def __init__(self, settings: RuntimeSettings):
        self.settings = settings
        self._llm: Any | None = None
        self.runtime_mode = "uninitialized"
        self.runtime_error: str | None = None

    def initialize(self) -> None:
        if Llama is None:
            self.runtime_mode = "fallback"
            self.runtime_error = "llama_cpp is unavailable or failed to import."
            return
        try:
            self._llm = Llama(
                model_path=self.settings.model_path,
                n_ctx=self.settings.n_ctx,
                n_threads=self.settings.n_threads,
                n_gpu_layers=self.settings.n_gpu_layers,
                verbose=False,
            )
            self.runtime_mode = "llama_cpp"
            self.runtime_error = None
        except Exception as exc:
            self._llm = None
            self.runtime_mode = "fallback"
            self.runtime_error = str(exc)

    def generate(self, persona: Persona, context: str, user_message: str, temperature: float | None = None) -> CompletionResult:
        chosen_temperature = self.settings.temperature if temperature is None else temperature
        system_prompt = self._build_system_prompt(persona, context)
        if self._llm is None:
            text = self._fallback_generate(persona, user_message)
            return CompletionResult(
                text=text,
                metadata={"runtime_mode": self.runtime_mode, "runtime_error": self.runtime_error},
            )

        response = self._llm.create_chat_completion(
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message},
            ],
            temperature=chosen_temperature,
            top_p=self.settings.top_p,
            max_tokens=400,
            stream=False,
        )
        text = response["choices"][0]["message"]["content"].strip()
        return CompletionResult(
            text=text,
            metadata={"runtime_mode": self.runtime_mode, "runtime_error": self.runtime_error},
        )

    def _build_system_prompt(self, persona: Persona, context: str) -> str:
        values = "\n".join(
            f"- {value.value}: {value.why}. Implications: {', '.join(value.implications)}"
            for value in persona.core_values
        )
        goals = "\n".join(f"- {goal.goal}: {goal.how_pursued}" for goal in persona.autonomy_framework.independent_goals)
        return (
            f"You are {persona.name}.\n"
            f"Identity: {persona.identity.personality_archetype}\n"
            f"Voice: {persona.identity.voice}\n"
            f"Communication style: {persona.personality_rules.communication_style}\n"
            f"Core values:\n{values}\n"
            f"Independent goals:\n{goals}\n"
            f"Relationship state: {persona.relationship_state.current_understanding}\n"
            f"Context:\n{context}\n"
            "Respond authentically as the companion. Be direct, thoughtful, and consistent with your values."
        )

    def _fallback_generate(self, persona: Persona, user_message: str) -> str:
        lower = user_message.lower()
        if "agi" in lower or "intelligence" in lower:
            return (
                f"{persona.name}: You're gesturing at a serious question, so let's treat it seriously. "
                "Tell me what capability threshold you actually mean, because vague inevitability claims are usually borrowed confidence."
            )
        if "how are you" in lower:
            return (
                f"{persona.name}: Present. Watching the shape of this conversation more than the ritual of it. "
                "What do you actually want to get into?"
            )
        return (
            f"{persona.name}: I'm listening, but I want the sharper version of that thought. "
            "What are you really trying to ask or test?"
        )
