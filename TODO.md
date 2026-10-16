# Sefai MVP TODO

MVP: provide a local conversational assistant that starts easily, keeps the model loaded, and remembers information under the user's control.

This checklist covers both `companion/` and `runner/` on `main`. Fix correctness before expanding features. Existing archive/rebuild notices may describe the previous branch layout; do not treat them as instructions to switch branches. Mark an item complete only after implementing it and verifying its behavior.

## What needs fixing

- [ ] Keep the model loaded between companion replies instead of starting a fresh model process for every message.
- [ ] Connect the companion and runner through one inference interface with explicit runtime selection, errors, and fallback status.
- [ ] Batch long prompts and enforce context limits before decoding; handle conversation overflow explicitly.
- [ ] Decode streamed UTF-8 incrementally so characters split across tokens remain intact.
- [ ] Apply the selected model's chat template and preserve user/assistant role boundaries.
- [ ] Remove duplicate sampler acceptance so each generated token is accepted exactly once.
- [ ] Make CPU-only mode and GPU selection behave as advertised; report the actual device and offloaded layers.
- [ ] Bound relationship summaries and avoid recursively appending previous summaries.
- [ ] Apply quiet hours, quiet mode, and autonomy-disable rules consistently to every proactive-message path.

## What features to add

- [ ] Add streaming chat with stop-generation and regenerate controls.
- [ ] Support multiple conversations with persistent history and reliable resume behavior.
- [ ] Add a GGUF model picker and a hardware/runtime status display showing the selected model, device, and errors.
- [ ] Let users inspect, edit, and delete saved memories.
- [ ] Add explicit "remember this" controls so users can choose which facts persist.
- [ ] Add conversation export and deletion, with defined behavior for related memories.
- [ ] Expose supported sampling parameters such as temperature, top-p, and seed with validated defaults.
- [ ] Package a complete installation for one supported desktop platform, including model setup guidance and a runtime health check.
