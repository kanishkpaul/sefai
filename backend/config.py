from __future__ import annotations

import os
from pathlib import Path
from pydantic import BaseModel, Field


DEFAULT_MODEL_PATH = Path(r"D:\Gemma-4-E2B-Uncensored-HauhauCS-Aggressive-Q4_K_P.gguf")


class RuntimeSettings(BaseModel):
    model_path: str = str(DEFAULT_MODEL_PATH)
    persona_path: str = str(Path("persona.sample.json").resolve())
    database_path: str = str(Path("runtime") / "companion.db")
    pipe_name: str = "sefai_companion_pipe"
    n_ctx: int = 4096
    n_threads: int = 8
    n_gpu_layers: int = 20
    temperature: float = 0.72
    top_p: float = 0.92
    autonomy_enabled: bool = True
    start_at_login: bool = True
    notifications_enabled: bool = True
    quiet_mode: bool = False
    ipc_version: str = Field(default="1.0")

    def runtime_dir(self) -> Path:
        return Path(self.database_path).resolve().parent

    @classmethod
    def from_environment(cls) -> "RuntimeSettings":
        values = {
            "model_path": os.getenv("SEFAI_MODEL_PATH", str(DEFAULT_MODEL_PATH)),
            "persona_path": os.getenv("SEFAI_PERSONA_PATH", str(Path("persona.sample.json").resolve())),
            "database_path": os.getenv("SEFAI_DATABASE_PATH", str(Path("runtime") / "companion.db")),
            "pipe_name": os.getenv("SEFAI_PIPE_NAME", "sefai_companion_pipe"),
            "n_ctx": int(os.getenv("SEFAI_N_CTX", "4096")),
            "n_threads": int(os.getenv("SEFAI_N_THREADS", "8")),
            "n_gpu_layers": int(os.getenv("SEFAI_N_GPU_LAYERS", "20")),
            "temperature": float(os.getenv("SEFAI_TEMPERATURE", "0.72")),
            "top_p": float(os.getenv("SEFAI_TOP_P", "0.92")),
            "autonomy_enabled": os.getenv("SEFAI_AUTONOMY_ENABLED", "true").lower() == "true",
            "start_at_login": os.getenv("SEFAI_START_AT_LOGIN", "true").lower() == "true",
            "notifications_enabled": os.getenv("SEFAI_NOTIFICATIONS_ENABLED", "true").lower() == "true",
            "quiet_mode": os.getenv("SEFAI_QUIET_MODE", "false").lower() == "true",
        }
        return cls(**values)
