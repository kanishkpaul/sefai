# Sefai Companion

Windows-native companion app with a WPF desktop shell and a Python local-LLM backend. It is designed around a persistent persona, local SQLite memory, refusal logic, autonomous outreach, and a GGUF model at `D:\Gemma-4-E2B-Uncensored-HauhauCS-Aggressive-Q4_K_P.gguf`.

## What is in this repo

- `backend/`: Python backend for persona logic, local memory, autonomous scheduling, model inference, and named-pipe IPC.
- `frontend/CompanionApp/`: WPF desktop app with chat UI, onboarding, settings, tray behavior, and backend process management.
- `persona.sample.json`: Editable sample persona in the schema described by the spec.
- `tests/`: Backend tests for persona validation, decision logic, memory retrieval, and autonomy triggers.

## Current status

- Backend tests pass on this machine with Python `3.12.10`.
- The local GGUF model path from the spec is the default runtime model path.
- The app now supports two local runtime paths:
  - `llama-cpp-python` if the native loader works on the machine.
  - official `llama.cpp` Windows CLI binaries as the fallback real-model runtime.

## Quick start

### 1. Install Python dependencies

```powershell
python -m pip install -r requirements.txt
```

### 2. Install .NET 8 desktop SDK

You need the Windows desktop SDK so the WPF app can build:

```powershell
winget install Microsoft.DotNet.SDK.8
```

After installation, confirm:

```powershell
dotnet --info
```

### 3. Run the backend directly

```powershell
python -m backend.main
```

### 4. Build and run the WPF shell

```powershell
dotnet build .\frontend\CompanionApp\CompanionApp.csproj
dotnet run --project .\frontend\CompanionApp\CompanionApp.csproj
```

## Behavior notes

- All data stays local in `runtime/companion.db`.
- The desktop app starts the backend process and talks to it through a Windows named pipe.
- On first launch, the desktop app automatically downloads the tested official `llama.cpp` Windows CPU runtime into `runtime_tools/llama_cpp/b9490/` if it is missing.
- Refusal and ignore decisions are deterministic and grounded in the persona.
- Autonomous messages are persisted in history and surfaced in the WPF timeline and tray notifications.
- If the `llama-cpp-python` native loader fails on a machine, the backend automatically falls back to the downloaded `llama.cpp` CLI runtime.
- If neither local model runtime is available, the backend still runs using a fallback responder so the app remains testable and exposes the runtime error in the UI.

## Verification

```powershell
python -m pytest tests
python -m compileall backend tests
```
