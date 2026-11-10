# GenUI

A Mac app whose UI DeepSeek writes while it runs. You describe an interface; DeepSeek writes a TSX module for
[Native Shell (NSH)](https://nsh.dev); the app checks and compiles it and shows it with AppKit's own views, in a
sandbox with no capabilities.

This is NSH's flow for generated UI ("a model writes a module, your server checks it, the app runs it in a
sandbox"), with the server's part done in the app for the demo.

## Run

```sh
scripts/bundle.sh          # builds target/GenUI.app
open target/GenUI.app
```

On first launch the settings open: paste a DeepSeek API key (from platform.deepseek.com). The app checks it
with DeepSeek's `/models` (free) and keeps it in the login keychain; neither the UI's JS nor generated code can
read it. ⌘, opens the settings again; there you also pick the model (`deepseek-flash`, the default, or `deepseek-v4-pro`).

Launch the app bundle, not the bare binary over SSH: a process started over SSH cannot write the login keychain
(`errSecWrPerm`, -61). The ad hoc signature changes with every build, so after a rebuild macOS may ask once
whether GenUI may read its key.

## How it works

```
shell/App.tsx (NSH, TSX)  ── host.call("generate") ──▶  src/generate.rs: a tool loop with DeepSeek (≤ 16 turns)
  prompt, versions, code view                             the model has two tools:
  ◀── host events: delta / status ──                        check(tsx)   compile + run headless, return errors
                                                            submit(tsx)  checked again, accepted only when it passes
                                                          check = src/build.rs (compile, imports against NSH's
                                                          exports) + src/verify.rs (headless NSH engine: render in
                                                          dark and light, press each enabled button, flag texts
                                                          whose WCAG contrast with what is behind them is < 3)
  <Sandbox program={…}>  ◀── { tsx, js } ──
     │
     ▼ src/sandbox.rs: native component "genui.sandbox"
  a second AppView: its own engine and QuickJS VM, embedding limits (256 MB heap, 2 s per call,
  200 000 nodes), capabilities { host: ["error"] } — no network, files or clipboard.
  A render error is caught by an ErrorBoundary and posted to the host → "让 DeepSeek 修复".
```

- `prompts/contract.md`: what a generated module must be (default-exported `App`, only `nsh` / `nsh/base` /
  `nsh/base/plot`, no capabilities). `prompts/nsh-tsx-guide.md` is NSH's own TSX guide, vendored at the NSH
  version in `Cargo.toml`; the two make the system prompt.
- The shell is compiled at start like generated code; its capabilities are in `shell/nsh.json`, which
  `nsh check shell` type-checks against too.

## Develop

```sh
nsh check shell                       # type-check the shell (nsh 0.4.0 CLI)
cargo test --release                  # compile/check rules
cargo test --release -- --ignored     # + DeepSeek with a wrong key (free), the keychain (needs a login session)
cargo run --release --example check -- file.tsx   # the model's check tool on a file
```

Integration tests (`tests/live.rs`) generate real UIs with DeepSeek and run each accepted module headless; no
window, no clicks. They read `DEEPSEEK_API_KEY` (and optionally `GENUI_TEST_MODEL`, `GENUI_TEST_THINKING`) and
write what was generated to `target/live/`:

```sh
DEEPSEEK_API_KEY=… cargo test --release --test live -- --ignored --nocapture
```

Measured with `deepseek-flash`: 5/5 cases pass (pomodoro and an edit of it, ledger, currency, weather, kanban),
39–160 s and 2–6 turns each; the model fixed the unreadable texts its checks reported in 3 of them.

`GENUI_DEV=1` reads commands from stdin (`src/dev.rs`): `prompt …`, `generate new|edit|fix`, `settings open|close`,
`tab preview|code`, `shot <png>`, `keychain-check`. `scripts/drive.sh` runs a sequence of them and quits;
`GENUI_REPLAY=<file>` answers generations with a file instead of DeepSeek (`examples/`):

```sh
GENUI_REPLAY=$PWD/examples/replay-pomodoro.md OPEN=1 scripts/drive.sh /tmp/genui.log \
  "prompt 番茄钟" "generate new" "wait 2" "shot $PWD/.shots/pomodoro.png"
```

## License

Apache-2.0. `prompts/nsh-*.md` are from [longbridge/nsh](https://github.com/longbridge/nsh) (Apache-2.0).
