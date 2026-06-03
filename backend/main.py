from __future__ import annotations

import asyncio
import logging
from pathlib import Path
import sys

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from backend.app import CompanionApplication
from backend.config import RuntimeSettings


def configure_logging() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s %(message)s",
        handlers=[
            logging.FileHandler("logs/backend.log", encoding="utf-8"),
            logging.StreamHandler(),
        ],
    )


async def async_main() -> None:
    settings = RuntimeSettings.from_environment()
    app = CompanionApplication(settings)
    await app.initialize()
    await app.run_forever()


def main() -> None:
    configure_logging()
    asyncio.run(async_main())


if __name__ == "__main__":
    main()
