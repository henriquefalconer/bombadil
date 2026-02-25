# Bombadil — Codebase Synthesis

Bombadil is a **coverage-guided property-based testing tool for web UIs** built by Antithesis. It drives a headless Chromium browser through randomized actions (clicks, typing, scrolling, navigation) while verifying **Linear Temporal Logic (LTL)** properties against the application's runtime state. Think of it as a fuzzer for web apps that can prove temporal invariants.

---

## Architecture Overview

```
User spec (.ts) ──→ Boa JS engine (Verifier) ──→ LTL Formulas + Action Generators
                                                          │
Browser (CDP) ←── Runner loop ──→ BrowserState snapshots ──→ Extractors ──→ Verifier.step()
     │                                  │                                        │
  Instrumented JS              Coverage edges                    Property values + Action tree
  (AFL-style)                  (guides exploration)              (weighted random pick)
                                                                        │
                                                              BrowserAction.apply() via CDP
```

### Core Loop (`src/runner.rs`)

The `Runner` is the central orchestrator. On each cycle it:

1. Receives a `BrowserEvent::StateChanged` with a full `BrowserState` snapshot.
2. Runs **extractors** — JS functions evaluated in the browser's paused debugger context — to capture application-specific state (DOM elements, input values, scroll position, etc.).
3. Calls `verifier.step()` to advance all LTL formulas by one time step and get an action tree.
4. Converts the JS action tree to `BrowserAction`s, filters to stay within the origin domain, prunes empty branches.
5. Picks a **weighted-random action** from the tree (guided by edge coverage).
6. Applies the action to the browser via CDP.
7. Stops if all properties are definite or a violation is found with `--exit-on-violation`.

---

## Module Breakdown

### `src/main.rs` — CLI Entry Point

Parses CLI arguments via `clap`:

- **`bombadil test <origin>`** — managed headless browser. Options: `--headless`, `--no-sandbox`, `--specification-file`, `--output-path`, `--exit-on-violation`, `--width`/`--height`/`--device-scale-factor`.
- **`bombadil test-external <origin> --remote-debugger <url>`** — connect to an already-running browser (e.g., Electron). Option: `--create-target`.

`Origin` accepts either a URL or a local file path (auto-prepends `file://`).

Exit codes: **0** = clean, **1** = error, **2** = violation found.

### `src/browser.rs` + `src/browser/` — Browser Automation

A **CDP state machine** managing a Chromium instance via `chromiumoxide`. Key components:

| File | Responsibility |
|---|---|
| `browser.rs` | State machine with states: `Pausing`, `Paused`, `Resuming`, `Navigating`, `Loading`, `Running`, `Acting`. Processes `InnerEvent` variants (Loaded, Paused, Resumed, FrameNavigated, NodeTreeModified, ConsoleEntry, ExceptionThrown, etc.). Emits `BrowserEvent::StateChanged`. 30s watchdog forces state capture on inactivity. |
| `actions.rs` | `BrowserAction` enum: `Back`, `Forward`, `Reload`, `Click { name, content, point }`, `TypeText { text, delay_millis }`, `PressKey { code }`, `ScrollUp/Down { origin, distance }`. Each variant implements `apply()` via CDP commands. |
| `evaluation.rs` | Evaluates JS expressions in a paused debugger call frame via `EvaluateOnCallFrameParams`. Handles null, bigint, exceptions. |
| `instrumentation.rs` | Intercepts network responses (Fetch domain) to instrument JS/HTML for coverage. Computes `SourceId` from etag or body hash. Writes debug copies to `/tmp/`. |
| `keys.rs` | Maps key codes to names (13→Enter, 27→Escape). |
| `state.rs` | `BrowserState` snapshot: URL, title, content type, console entries, exceptions, navigation history, edge coverage (AFL-style bucket deltas), screenshot, transition hash (simhash). |

### `src/instrumentation/` — AFL-Style Edge Coverage

Injects `window.__bombadil__` with two `Uint8Array(65536)` edge maps and a `previous` counter. At each branch point:

```js
__bombadil__.edges_current[(HASH ^ __bombadil__.previous) % 65536] += 1;
__bombadil__.previous = HASH >> 1;
```

| File | What it instruments |
|---|---|
| `js.rs` | JavaScript source code via `oxc` AST transformer. Hooks: `if`/`else`, `switch`/`case`, ternary `? :`, `for`/`for-in`/`for-of` loops. |
| `html.rs` | Inline `<script>` tags in HTML documents (delegates to `js.rs` per script). |
| `source_id.rs` | `SourceId(u64)` — hash-based unique identifiers per source file to avoid coverage collisions. Supports `.add()` for inline script disambiguation. |

### `src/specification/` — Specification Engine

**Rust side:**

| File | Purpose |
|---|---|
| `verifier.rs` | `Verifier` loads user specs via Boa JS engine. Exports that are `Formula` instances become **properties**; `ActionGenerator` instances become **action generators**. `step()` evaluates/advances all formulas and calls all generators. `Specification` struct handles `.ts` transpilation. |
| `worker.rs` | `VerifierWorker` wraps the non-`Send` Boa engine in a dedicated OS thread with `mpsc` channels. Exposes async `properties()`, `extractors()`, `step()`. |
| `js.rs` | Bridge between Boa and Rust types. `JsAction` (camelCase serde for JS interop) ↔ `BrowserAction` conversion. `RuntimeFunction` wraps `JsObject` + pretty string. `Syntax::from_value()` uses `instanceof` checks against bombadil formula classes. `BombadilExports` holds references to all formula constructors. `Extractors` registry with `register()`, `extract_functions()`, `update_from_snapshots()`. |
| `syntax.rs` | `Syntax<Function>` — pre-NNF formula AST with `Not` variant. `.nnf()` pushes negations inward via De Morgan's laws and temporal duality (`¬□ = ◇¬`, `¬◇ = □¬`). |
| `ltl.rs` | Core LTL evaluator with three-valued semantics: `True`, `False(Violation)` (with proof tree), `Residual` (continuation). `Evaluator::evaluate()` for initial formulas, `Evaluator::step()` for advancing residuals. Temporal operators (Always, Eventually) with optional `Duration` bounds. `Violation` is a recursive proof tree (And, Or, Implies, Always, Eventually variants). |
| `stop.rs` | `stop_default()` — determines truth value of residuals when test ends. Always-that-hasn't-been-violated → True; Eventually-not-yet-satisfied → False. |
| `render.rs` | Renders `Violation` and `Formula` trees to human-readable strings. Maps `RuntimeFunction` → `PrettyFunction(String)`. |
| `module_loader.rs` | Custom Boa module loader. `HybridModuleLoader` combines: (1) `MapModuleLoader` for built-in bombadil modules (embedded at compile time from `target/specification/`), (2) `SimpleModuleLoader` for filesystem modules. TypeScript transpilation via `oxc` (parser + SemanticBuilder + Transformer). |
| `result.rs` | `SpecificationError` enum: JS, IO, TranspilationError, SystemTimeError, OtherError. |

**TypeScript side (the user-facing DSL):**

| File | Public API |
|---|---|
| `index.ts` | Main module `@antithesishq/bombadil`. Exports: `extract()`, `always()`, `eventually()`, `next()`, `now()`, `not()`, `actions()`, `weighted()`. Defines `State` type (document, window, navigationHistory, errors, console, lastAction). LTL formula class hierarchy: `Formula` → `Pure`, `Thunk`, `Not`, `And`, `Or`, `Implies`, `Next`, `Always` (with `.within(n, unit)`), `Eventually` (with `.within(n, unit)`). |
| `internal.ts` | Reactive cell system. `Cell<T>` interface (`.current`, `.at(time)`, `.update()`). `ExtractorCell<T, S>` stores snapshots in a `Map<Time, T>`, serializes its extract function via `.asJsFunction()`. `TimeCell` tracks current time. `Runtime<S>` registers all extractors. |
| `actions.ts` | `Action` tagged union (Back, Forward, Reload, Click, TypeText, PressKey, ScrollUp, ScrollDown). `Tree<T>` weighted tree type. `ActionGenerator` class. `actions()` and `weighted()` factory functions. Re-exports random generators. |
| `random.ts` | Backed by Rust-provided `__bombadil_random_bytes`. Generators: `strings()`, `emails()`, `integers()`, `keycodes()`, `from(elements)`, `randomRange(min, max)`. |
| `defaults.ts` | Barrel re-export of default properties + actions. |
| `defaults/properties.ts` | `noHttpErrorCodes` (status < 400), `noUncaughtExceptions`, `noUnhandledPromiseRejections`, `noConsoleErrors` — all wrapped in `always()`. |
| `defaults/actions.ts` | `clicks` (recursive DOM scan across shadow roots + iframes for clickable elements), `inputs` (typing into focused text/email/number fields), `scroll` (based on current position), `navigation` (weighted: back=10, forward=1, reload=1). |

### `src/tree.rs` — Weighted Tree

`Tree<T>` = `Leaf { value }` | `Branch { branches: Vec<(Weight, Tree)> }` where `Weight = u16`.

Methods: `try_map()`, `filter()`, `prune()` (removes empty branches), `pick(rng)` (weighted random walk from root).

### `src/trace/` — Trace Output

| File | Purpose |
|---|---|
| `mod.rs` | `TraceEntry` (timestamp, url, hashes, action, screenshot path, violations). `PropertyViolation` (name + rendered violation). |
| `writer.rs` | `TraceWriter` — saves screenshots as files (named by microsecond timestamp), appends `TraceEntry` as JSON lines to `trace.jsonl`. |

### `src/geometry.rs` — Point Type

`Point { x: f64, y: f64 }` with `From` conversions to/from `chromiumoxide::layout::Point`.

### `src/url.rs` — URL Utilities

`is_within_domain(uri, domain)` — checks host+port match. Used by runner to constrain navigation.

### `src/build.rs` — Build Script

Compiles `src/specification/*.ts` → `target/specification/*.js` via `esbuild --format=esm`. The built JS is embedded into the binary at compile time via `include_dir!` in `module_loader.rs`.

---

## Tests

### Integration Tests (`tests/integration_tests.rs`)

11 end-to-end tests using real headless Chrome + Axum HTTP servers. Each test:

1. Serves HTML fixtures from `tests/<name>/` on two ports (P and P+1 for cross-domain tests).
2. Creates a `Specification` (default: re-export `@antithesishq/bombadil/defaults`; some tests use custom specs).
3. Creates and starts a `Runner` with managed headless Chrome.
4. Processes `RunEvent::NewState` events, matching outcome against `Expect::Error { substring }` or `Expect::Success`.

| Test | Fixture | Expects | What it validates |
|---|---|---|---|
| `test_console_error` | `console-error/` | Error: "noConsoleErrors" | Button triggers `console.error` after 3 clicks |
| `test_links` | `links/` | Error: "noHttpErrorCodes" | Broken link to `d.html` (404) |
| `test_uncaught_exception` | `uncaught-exception/` | Error: "noUncaughtExceptions" | Button throws after 3 clicks |
| `test_unhandled_promise_rejection` | `unhandled-promise-rejection/` | Error: "noUnhandledPromiseRejections" | Button rejects promise after 3 clicks |
| `test_other_domain` | `other-domain/` | Success | Fuzzer stops interacting with cross-origin pages |
| `test_action_within_iframe` | `action-within-iframe/` | Success | Fuzzer clicks buttons inside iframes |
| `test_no_action_available` | `no-action-available/` | Error: "no actions available" | Empty page with no clickable elements |
| `test_back_from_non_html` | `back-from-non-html/` | Success | Navigate to XML feed and back |
| `test_random_text_input` | `random-text-input/` | Success | Fuzzer types into text inputs |
| `test_counter_state_machine` | `counter-state-machine/` | Success | `always(unchanged ∨ increment ∨ decrement)` holds |
| `test_browser_lifecycle` | (no fixture) | Success | Direct `Browser` API test (initiate, events, apply, terminate) |

### Unit / Snapshot Tests

- **Instrumentation snapshots** (`src/instrumentation/snapshots/`): 11 `insta` snapshots verifying JS/HTML instrumentation output for if, if-else, switch, ternary (plain, assignment, await, comma operator), and HTML inline scripts (no type, javascript type, other type).
- **LTL equivalence tests** (`src/specification/ltl_equivalences.rs`): `proptest`-based verification of distributivity, negation duality, and idempotency.
- **Random range tests** (`src/specification/random_test.rs`): `proptest` verification that `randomRange(min, max)` always returns integers in `[min, max)`.
- **Tree tests** (`src/tree.rs`): prune, filter, try_map, pick.
- **URL tests** (`src/url.rs`): domain matching, relative URL parsing.
- **Verifier tests** (`src/specification/verifier.rs`): property evaluation for all LTL operators, TypeScript spec loading.

---

## Key Design Decisions

1. **Boa JS engine on a dedicated thread** — Boa is not `Send`, so `VerifierWorker` runs it on a single OS thread with `mpsc` channels, exposing an async interface.
2. **AFL-style edge coverage** — instrumented into the application's JS at network interception time (not at build time), making it transparent to the app under test.
3. **Three-valued LTL** — formulas can be True, False (with proof tree), or Residual (undecided). This enables early termination when all properties become definite.
4. **Negation Normal Form** — user-facing `Syntax` supports `Not`; it's pushed inward via De Morgan + temporal duality before evaluation, so the evaluator only handles NNF.
5. **TypeScript specs transpiled at runtime** — user specs can be `.ts` files; `oxc` strips types at runtime, and built-in modules are pre-compiled via `esbuild` at build time.
6. **Weighted action trees** — action generators return `Tree<Action>` with weights, allowing fine-grained control over action probability (e.g., `back=10, forward=1, reload=1`).
7. **Debugger-based state capture** — the browser's JS debugger is paused to capture a consistent state snapshot, ensuring no mutations occur during extraction.
