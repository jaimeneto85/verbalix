# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Verbalix is a macOS 14+ menu-bar (accessory) app that transforms text selected in *any* other application: PT↔EN translation and technical-writing improvement. Stack: Tauri 2 + Rust core, React 19 + TypeScript + Vite frontend, Supabase (Auth magic link, Postgres history, Edge Function that proxies OpenAI).

## Commands

```bash
npm run dev                 # Vite dev server on :1420 (strict port)
npm run build               # tsc && vite build → dist/
npm test                    # vitest run (frontend)
npm run test:coverage       # coverage; thresholds enforced on src/native.ts + src/types.ts only
npm run test:e2e            # Playwright against the Vite server with a stubbed Tauri adapter
npx vitest run src/native.test.ts          # single frontend test file
npx vitest run -t "applies a preview"      # single test by name
npx playwright test e2e/ai-readiness.e2e.ts

cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml coordinator   # filter by name
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml

deno test supabase/functions/transform/contract_test.ts

npm run tauri -- dev                          # full desktop app
npm run tauri -- build --debug --bundles app  # bundle smoke
```

Full gate set before handoff: `npm test`, `npm run test:coverage`, `npm run test:e2e`, `npm run build`, `cargo test`, `cargo clippy -D warnings`, deno test, and the debug bundle.

npm is the package manager (`package-lock.json` is the committed lockfile).

## Environment

- Canonical names, used by **both** Vite and the Rust build: `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY` in a root `.env` — see `.env.example`.
- `build.rs` reads the complete pair from `.env` and embeds the public config into `OUT_DIR` (never via stdout or `cargo:rustc-env`); complete process env vars win in development. `VERBALIX_SUPABASE_URL` / `VERBALIX_SUPABASE_ANON_KEY` are legacy aliases only and are never partially merged with the canonical pair. Resolution order lives in `application/ai_readiness.rs`.
- Missing config is not an error: `ai_readiness` reports `provider_not_configured` / `login_required` / `ready`, and the UI routes the user to the main window.
- Edge Function (Supabase secrets): `OPENAI_API_KEY`, `OPENAI_MODEL`. The OpenAI key never reaches the client.

## Architecture

Hexagonal in Rust. Three layers under `src-tauri/src/`, dependencies point inward:

- `domain/` — pure logic, no Tauri/macOS/Supabase: `SelectionSnapshot`/`SelectionState`/`SelectionEvent`, settings, transform contracts (`AiProvider` trait), `VerbalixError`.
- `application/` — `SelectionCoordinator` (the state machine), ports (`SelectionPort`, `OverlayPort`, `ClipboardPort`), `ai_readiness.rs` (public backend config resolution + readiness/refresh-failure classification), and remote adapters (`RemoteTransformer`, `RemoteHistoryRepository`, `RemoteAuthRepository`, `KeychainSessionRepository`, `JsonSettingsRepository`, `RuntimePause`).
- `platform/` — macOS adapters behind `cfg(target_os = "macos")`: AXUIElement capture/replace, AXObserver, `macos_geometry.rs` (selection bounds → AX element frame → cursor fallback chain), clipboard fallback, `TauriOverlay` + `MainThreadOverlayDispatcher`. A non-macOS stub in `platform/mod.rs` keeps the crate compiling elsewhere.
- `diagnostics.rs` — opt-in tracing gated by `VERBALIX_DIAGNOSTICS=1`. Sanitized by construction: origin, UUID, PID, UTF-16 range, bounds, writability, sequence, visibility, error codes. Never the selected text; tests enforce this.

`lib.rs` wires everything in `tauri::Builder::setup`, stores `Arc<AppRuntime>` in Tauri state, registers the tray, the global shortcut, the AXObserver callback, the mouse-dismiss monitor and the polling thread. `commands.rs` holds every `#[tauri::command]`; the frontend touches them only through `src/native.ts` (the single IPC boundary — typed in `src/types.ts`, camelCase over the wire).

### Invariants to preserve

- **Overlay work must run on the main thread.** AXObserver callbacks and the polling thread run in background threads; every NSWindow/NSPanel operation goes through `MainThreadOverlayDispatcher` (`AppHandle::run_on_main_thread`) as an `OverlayCommand`. Calling AppKit directly from those callbacks crashed the app before (`docs/003`). Never bypass the dispatcher, and never block the callback waiting on the UI.
- **Latest-wins + revalidation.** Snapshots are immutable; every write revalidates the current snapshot with `same_target` and the active `request_id`. Any failure after `Processing` funnels through `fail`/`recover_request` back to `ToolbarVisible` — non-destructive, no stale response ever applied.
- **`RuntimePause` is the single gate** for polling, AXObserver, global shortcut and clipboard fallback. Callbacks re-check `is_paused()` after the 150 ms debounce.
- **Note results use publish-then-emit.** The backend stores the payload in `NoteResultState` before emitting; the frontend registers its listener before calling `current_note_result`, so results created before listener readiness are not lost.
- **Never log or return selected text / secrets.** External failures are mapped to `VerbalixError` variants without content. Sessions live in Keychain; only non-sensitive prefs go to `settings.json` in the app config dir.
- **Non-activating panel config stays inside the AppKit boundary.** `WebviewWindow::set_focusable(false)` after the dynamic class swap to `NSPanel` reaches a missing ivar and kills the process (`docs/005`). Configure focus behaviour natively, not through the Tauri setter.
- **AX geometry failures must not produce the `(0,0,1,1)` sentinel.** Fall through selection bounds → AX element frame → global cursor position.

### Frontend

`src/main.tsx` renders one of two roots from the same bundle based on `?overlay=` in the URL: `Overlay` (toolbar/note panels, opened as separate Tauri windows) or `App` (settings + history window). Window labels `main`, `toolbar`, `note` are declared in `src-tauri/capabilities/default.json` — adding a window requires updating that capability. Styles are split by concern under `src/styles/`.

Supabase magic-link auth uses PKCE; the callback arrives through the `verbalix://` deep link, is exchanged in `App.tsx`, and the session is handed to Rust via `save_session` for Keychain storage.

The Playwright suite in `e2e/` drives the real Vite build with `window.__TAURI_INTERNALS__` stubbed in an init script, asserting on recorded `invoke` calls. It proves UI routing and command sequencing, not native behaviour.

### Edge Function

`supabase/functions/transform/` — `contract.ts` (request parsing + `ErrorCode`s, unit-tested), `provider.ts` (`AiProvider` → OpenAI Responses API), `index.ts` (JWT-required handler, 20 s abort). History table + RLS (owner-only, 30-day retention) in `supabase/migrations/`.

## Conventions

- Keep files under ~300 effective lines; split by responsibility (see how `platform/` and `styles/` are already split).
- Rust types crossing IPC serialize as camelCase.
- Docs are written in Portuguese under `docs/NNN-*.md` (one per delivered scope); SDD plans live in `tasks/<task-name>/plan.md`.

## Known manual gates

AX-dependent behaviour cannot be automated here: real-app validation (Chrome, Safari, VS Code, Slack, Notes, TextEdit), full clipboard restoration in a real process, multi-monitor/fullscreen `NSPanel` clamping, and a genuine end-to-end AI transform (needs a deployed Edge Function plus an authenticated session). These require a signed bundle plus Accessibility permission — do not claim them as verified from unit or Playwright tests.

The bundle is ad-hoc signed (`"signingIdentity": "-"`), so it has no stable TCC identity across rebuilds: an existing Accessibility entry can stay enabled while `AXIsProcessTrusted` returns false for the current bundle. Rebuild-then-missing-toolbar is usually stale TCC, not a code defect (`docs/004`). Never auto-reset TCC.
