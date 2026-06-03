from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any, Literal

from pydantic import BaseModel, Field


class PersonaValue(BaseModel):
    value: str
    why: str
    implications: list[str]


class IndependentGoal(BaseModel):
    goal: str
    how_pursued: str
    trigger_condition: str


class RefusalRule(BaseModel):
    condition: str
    response: str


class TriggerRule(BaseModel):
    name: str
    condition: str
    probability: float | None = None
    days: int | None = None
    action: str | None = None
    example: str | None = None


class Identity(BaseModel):
    age_presentation: str
    personality_archetype: str
    voice: str
    aesthetic: str


class PersonalityRules(BaseModel):
    communication_style: str
    emotional_range: list[str]
    humor: str
    stance_on_philosophy: str


class AutonomyFramework(BaseModel):
    independent_goals: list[IndependentGoal]
    refusal_rules: list[RefusalRule]


class MessageTriggers(BaseModel):
    time_based: list[TriggerRule] = Field(default_factory=list)
    state_based: list[TriggerRule] = Field(default_factory=list)
    goal_based: list[TriggerRule] = Field(default_factory=list)


class RelationshipState(BaseModel):
    current_understanding: str
    what_i_respect: list[str]
    what_i_question: list[str]
    how_we_relate: str


class MoodRules(BaseModel):
    default_mood: str = "engaged"
    withdrawn_after_surface_messages: int = 2
    reengage_after_substantive_messages: int = 1


class QuietHours(BaseModel):
    enabled: bool = True
    start: str = "22:00"
    end: str = "08:00"


class UiProfile(BaseModel):
    accent_color: str = "#D08770"
    avatar_label: str = "A"


class Persona(BaseModel):
    name: str
    identity: Identity
    core_values: list[PersonaValue]
    personality_rules: PersonalityRules
    autonomy_framework: AutonomyFramework
    message_triggers: MessageTriggers
    relationship_state: RelationshipState
    mood_rules: MoodRules = Field(default_factory=MoodRules)
    quiet_hours: QuietHours = Field(default_factory=QuietHours)
    ui_profile: UiProfile = Field(default_factory=UiProfile)


class MessageRecord(BaseModel):
    role: Literal["user", "companion"]
    content: str
    initiated_by: Literal["user", "autonomous", "system"] | None = None
    timestamp: datetime | None = None
    persona_state: dict[str, Any] | None = None


class AppState(BaseModel):
    mood: str
    relationship_summary: str
    active_goals: list[str]
    autonomy_enabled: bool
    quiet_mode: bool
    last_user_message_at: datetime | None = None


class MessageEnvelope(BaseModel):
    type: str
    request_id: str | None = None
    payload: dict[str, Any] = Field(default_factory=dict)
    error: str | None = None


@dataclass(slots=True)
class MessageDecision:
    should_refuse: bool = False
    reason: str = ""
    refusal_response: str | None = None
    engagement_score: float = 0.0
