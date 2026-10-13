# Abilities

An ability is one self-contained capability of the companion: who it is, what it remembers, when it speaks first.
Each ability is its own crate under `abilities/`, compiled in by a Cargo feature and switched on by configuration.
The point of the design is that adding or removing one is a small, local edit that cannot break the others.

## How an ability plugs in

An ability implements `sefai_core::Ability`:

```rust
pub trait Ability: Send {
    fn name(&self) -> &'static str;
    fn prepare(&mut self, prompt: &mut Prompt) -> Result<(), AbilityError>;
}
```

- `name` is a short, stable, lowercase identifier. The crate also exports it as `pub const NAME`.
- `prepare` runs before every turn. It may read the conversation and add system sections with `prompt.add_section(NAME, text)`. It cannot change the conversation.

The runtime runs abilities in the order of the registry in `crates/sefai/src/abilities.rs`, so that order is also the order their text appears in the system prompt.

If `prepare` returns an error, the turn still happens. Anything that ability added is removed, the remaining abilities run, and the failure is logged.

More hooks are planned, each arriving with the first ability that needs it: `observe` after each exchange (memory, phase 3) and `tick` on a schedule (autonomy, phase 4). Hooks that an ability doesn't need will have default implementations that do nothing.

## Adding an ability

Say the new ability is `mood`.

1. **Create the crate** at `abilities/mood/` with package name `sefai-ability-mood`. The workspace picks up anything under `abilities/` automatically. Depend on `sefai-core`, and on `sefai-store` only if it persists data. Never depend on another ability, an engine or the runtime.
2. **Implement the trait** in `abilities/mood/src/lib.rs`, export `pub const NAME: &str = "mood";`, and write tests that drive it through `prepare` on a `Prompt`.
3. **Add it to the workspace dependencies** in the root `Cargo.toml`:
   ```toml
   sefai-ability-mood = { path = "abilities/mood" }
   ```
4. **Add a feature** in `crates/sefai/Cargo.toml`, and list it in `default` if it should ship by default:
   ```toml
   mood = ["dep:sefai-ability-mood"]
   ```
   and the optional dependency:
   ```toml
   sefai-ability-mood = { workspace = true, optional = true }
   ```
5. **Register it** in `crates/sefai/src/abilities.rs`: a row in `REGISTRY` and a build function, both under `#[cfg(feature = "mood")]`. If it needs settings, add a field to `Options` under the same `cfg`.
   ```rust
   #[cfg(feature = "mood")]
   Entry { name: sefai_ability_mood::NAME, build: mood },
   ```
   The build function returns `Ok(None)` when the options leave the ability switched off.
6. **Check both ways:** `cargo test --workspace`, and `cargo clippy -p sefai --no-default-features -- -D warnings` so the binary still builds without it.

## Removing an ability

Reverse the steps: delete the `REGISTRY` row, build function and `Options` field, the feature and optional dependency in `crates/sefai/Cargo.toml`, the workspace dependency, and the crate directory.
Nothing outside those places should need to change. If something does, an invariant in `ARCHITECTURE.md` was broken and is worth fixing first.

To switch an ability off without removing it, build without its feature (`cargo build --no-default-features --features ...`) or turn it off in configuration once phase 2 adds the config file.

## Rules for abilities

- Do one thing. If the description needs "and", consider two abilities.
- Keep contributions short. Every token added to the prompt is prefilled on every turn whose prefix it changes, so prefer stable text that stays cached.
- Put stable text before volatile text. Sections that rarely change (persona) belong earlier in the registry than ones that change every turn (recalled memories), so the KV cache prefix survives.
- Never block for long in a hook. Hooks run on the turn's critical path. Expensive work belongs in `observe` or `tick`, once those exist.
- Store nothing outside `sefai-store`. No files of your own, no network.
- Test through the trait: build a `Prompt`, call `prepare`, assert on `sections()`.
