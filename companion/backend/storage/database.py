from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Any

import aiosqlite

from backend.models import AppState, MessageRecord, Persona


DATABASE_SCHEMA = """
CREATE TABLE IF NOT EXISTS persona (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    payload TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    initiated_by TEXT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    persona_state TEXT
);

CREATE TABLE IF NOT EXISTS memory_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    summary TEXT NOT NULL,
    relevant_to TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    importance_score REAL DEFAULT 0.5
);

CREATE TABLE IF NOT EXISTS autonomous_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger_type TEXT NOT NULL,
    trigger_condition TEXT NOT NULL,
    scheduled_time TIMESTAMP,
    status TEXT DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    delivered_message TEXT
);

CREATE TABLE IF NOT EXISTS refusals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_message TEXT NOT NULL,
    reason TEXT NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS state_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mood TEXT NOT NULL,
    relationship_summary TEXT NOT NULL,
    active_goals TEXT NOT NULL,
    autonomy_enabled INTEGER NOT NULL,
    quiet_mode INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
"""


class Database:
    def __init__(self, path: str):
        self.path = Path(path)
        self.connection: aiosqlite.Connection | None = None

    async def connect(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.connection = await aiosqlite.connect(self.path)
        self.connection.row_factory = aiosqlite.Row
        await self.connection.executescript(DATABASE_SCHEMA)
        await self.connection.commit()

    async def close(self) -> None:
        if self.connection:
            await self.connection.close()
            self.connection = None

    def _conn(self) -> aiosqlite.Connection:
        if not self.connection:
            raise RuntimeError("Database connection has not been initialized.")
        return self.connection

    async def save_persona(self, persona: Persona) -> None:
        payload = json.dumps(persona.model_dump(mode="json"))
        await self._conn().execute(
            """
            INSERT INTO persona (id, payload, updated_at)
            VALUES (1, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = CURRENT_TIMESTAMP
            """,
            (payload,),
        )
        await self._conn().commit()

    async def save_message(self, message: MessageRecord) -> None:
        await self._conn().execute(
            """
            INSERT INTO messages (role, content, initiated_by, persona_state)
            VALUES (?, ?, ?, ?)
            """,
            (
                message.role,
                message.content,
                message.initiated_by,
                json.dumps(message.persona_state) if message.persona_state else None,
            ),
        )
        await self._conn().commit()

    async def fetch_recent_messages(self, limit: int = 20) -> list[dict[str, Any]]:
        cursor = await self._conn().execute(
            """
            SELECT role, content, initiated_by, timestamp, persona_state
            FROM messages
            ORDER BY id DESC
            LIMIT ?
            """,
            (limit,),
        )
        rows = await cursor.fetchall()
        result = []
        for row in reversed(rows):
            result.append(
                {
                    "role": row["role"],
                    "content": row["content"],
                    "initiated_by": row["initiated_by"],
                    "timestamp": row["timestamp"],
                    "persona_state": json.loads(row["persona_state"]) if row["persona_state"] else None,
                }
            )
        return result

    async def get_last_user_message_at(self) -> datetime | None:
        cursor = await self._conn().execute(
            "SELECT MAX(timestamp) AS ts FROM messages WHERE role = 'user'"
        )
        row = await cursor.fetchone()
        if not row or not row["ts"]:
            return None
        return datetime.fromisoformat(str(row["ts"]))

    async def save_memory_summary(self, summary: str, relevant_to: str, importance_score: float) -> None:
        await self._conn().execute(
            """
            INSERT INTO memory_summaries (summary, relevant_to, importance_score)
            VALUES (?, ?, ?)
            """,
            (summary, relevant_to, importance_score),
        )
        await self._conn().commit()

    async def fetch_memory_summaries(self, limit: int = 5) -> list[dict[str, Any]]:
        cursor = await self._conn().execute(
            """
            SELECT summary, relevant_to, created_at, importance_score
            FROM memory_summaries
            WHERE importance_score > 0.3
            ORDER BY id DESC
            LIMIT ?
            """,
            (limit,),
        )
        rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    async def log_refusal(self, user_message: str, reason: str) -> None:
        await self._conn().execute(
            "INSERT INTO refusals (user_message, reason) VALUES (?, ?)",
            (user_message, reason),
        )
        await self._conn().commit()

    async def enqueue_autonomous_message(
        self,
        trigger_type: str,
        trigger_condition: str,
        scheduled_time: datetime,
        delivered_message: str,
    ) -> int:
        cursor = await self._conn().execute(
            """
            INSERT INTO autonomous_queue (trigger_type, trigger_condition, scheduled_time, status, delivered_message)
            VALUES (?, ?, ?, 'pending', ?)
            """,
            (trigger_type, trigger_condition, scheduled_time.isoformat(), delivered_message),
        )
        await self._conn().commit()
        return int(cursor.lastrowid)

    async def mark_autonomous_message_sent(self, queue_id: int) -> None:
        await self._conn().execute(
            "UPDATE autonomous_queue SET status = 'sent' WHERE id = ?",
            (queue_id,),
        )
        await self._conn().commit()

    async def record_state_snapshot(self, state: AppState) -> None:
        await self._conn().execute(
            """
            INSERT INTO state_snapshots (mood, relationship_summary, active_goals, autonomy_enabled, quiet_mode)
            VALUES (?, ?, ?, ?, ?)
            """,
            (
                state.mood,
                state.relationship_summary,
                json.dumps(state.active_goals),
                int(state.autonomy_enabled),
                int(state.quiet_mode),
            ),
        )
        await self._conn().commit()

    async def fetch_latest_state(self) -> AppState | None:
        cursor = await self._conn().execute(
            """
            SELECT mood, relationship_summary, active_goals, autonomy_enabled, quiet_mode
            FROM state_snapshots
            ORDER BY id DESC
            LIMIT 1
            """
        )
        row = await cursor.fetchone()
        if not row:
            return None
        return AppState(
            mood=row["mood"],
            relationship_summary=row["relationship_summary"],
            active_goals=json.loads(row["active_goals"]),
            autonomy_enabled=bool(row["autonomy_enabled"]),
            quiet_mode=bool(row["quiet_mode"]),
        )

    async def save_setting(self, key: str, value: Any) -> None:
        await self._conn().execute(
            """
            INSERT INTO app_settings (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """,
            (key, json.dumps(value)),
        )
        await self._conn().commit()

    async def fetch_settings(self) -> dict[str, Any]:
        cursor = await self._conn().execute("SELECT key, value FROM app_settings")
        rows = await cursor.fetchall()
        return {row["key"]: json.loads(row["value"]) for row in rows}

    async def log_event(self, event_type: str, payload: dict[str, Any]) -> None:
        await self._conn().execute(
            "INSERT INTO events (event_type, payload) VALUES (?, ?)",
            (event_type, json.dumps(payload)),
        )
        await self._conn().commit()
