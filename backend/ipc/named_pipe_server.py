from __future__ import annotations

import asyncio
import json
import threading
from typing import Awaitable, Callable

from backend.models import MessageEnvelope

try:
    import pywintypes  # type: ignore
    import win32file  # type: ignore
    import win32pipe  # type: ignore
except ImportError:  # pragma: no cover
    pywintypes = None
    win32file = None
    win32pipe = None


EnvelopeHandler = Callable[[MessageEnvelope], Awaitable[MessageEnvelope]]


class NamedPipeServer:
    def __init__(self, pipe_name: str, handler: EnvelopeHandler):
        self.pipe_name = pipe_name
        self.handler = handler
        self._stop_event = threading.Event()
        self._thread: threading.Thread | None = None
        self._loop: asyncio.AbstractEventLoop | None = None

    async def start(self) -> None:
        if win32pipe is None or win32file is None:
            raise RuntimeError("pywin32 is required to run the named pipe server on Windows.")
        self._loop = asyncio.get_running_loop()
        self._thread = threading.Thread(target=self._serve_forever, name="NamedPipeServer", daemon=True)
        self._thread.start()

    async def stop(self) -> None:
        self._stop_event.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2)

    def _serve_forever(self) -> None:  # pragma: no cover
        assert self._loop is not None
        pipe_path = rf"\\.\pipe\{self.pipe_name}"
        while not self._stop_event.is_set():
            pipe = win32pipe.CreateNamedPipe(
                pipe_path,
                win32pipe.PIPE_ACCESS_DUPLEX,
                win32pipe.PIPE_TYPE_MESSAGE | win32pipe.PIPE_READMODE_MESSAGE | win32pipe.PIPE_WAIT,
                win32pipe.PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                None,
            )
            try:
                win32pipe.ConnectNamedPipe(pipe, None)
                while not self._stop_event.is_set():
                    try:
                        _, data = win32file.ReadFile(pipe, 65536)
                    except pywintypes.error:
                        break
                    raw = data.decode("utf-8").strip()
                    if not raw:
                        continue
                    request = MessageEnvelope.model_validate(json.loads(raw))
                    future = asyncio.run_coroutine_threadsafe(self.handler(request), self._loop)
                    response = future.result()
                    payload = (response.model_dump_json() + "\n").encode("utf-8")
                    win32file.WriteFile(pipe, payload)
            finally:
                win32file.CloseHandle(pipe)
