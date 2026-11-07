//! GenUI: a Mac app whose UI DeepSeek writes at run time.
//!
//! The window is an NSH app (`shell/App.tsx`, compiled here at start). Its `host.call`s come to this process:
//! `generate` asks DeepSeek for a TSX module and checks it (`calls.rs`, on the library's `generate.rs`, `build.rs`), `settings.*` keep the
//! user's API key in the keychain (`keychain.rs`). The shell shows each module in `genui.sandbox`
//! (`sandbox.rs`), a second NSH engine with no capabilities.

mod calls;
mod dev;
mod keychain;
mod sandbox;

use std::process::ExitCode;

use nsh_engine::HostChannel;
use nsh_genui::{build, deepseek};
use nsh_platform_macos::{AppOptions, AppSource, Bundle, Components, run_app};
use serde_json::{Value, json};

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,genui=info")).init();

    let shell = match build::shell(include_str!("../shell/App.tsx")) {
        Ok(js) => js,
        Err(e) => {
            eprintln!("shell/App.tsx does not compile:\n{e}");
            return ExitCode::FAILURE;
        }
    };
    // The shell's capabilities have one home, its nsh.json (which `nsh check shell` reads too).
    let manifest: Value = serde_json::from_str(include_str!("../shell/nsh.json")).expect("shell/nsh.json");
    let capabilities = manifest["capabilities"].to_string();
    let bundle = Bundle::source(shell)
        .with_capabilities_json(Some(&capabilities))
        .expect("shell capabilities");

    let host = HostChannel::default();
    handle_calls(&host);
    if dev::enabled() {
        dev::start(host.clone());
    }
    let components = Components::default();
    sandbox::register(&components);

    run_app(
        AppSource::Bundle(bundle),
        AppOptions {
            title: "GenUI".to_owned(),
            app_id: manifest["id"].as_str().unwrap_or("dev.felix.nsh-genui").to_owned(),
            size: (1360, 880),
            host,
            components,
            ..AppOptions::default()
        },
    )
}

/// The shell's calls. Each runs off the engine thread: they wait on the network or the keychain.
fn handle_calls(host: &HostChannel) {
    let channel = host.clone();
    host.handle("generate", move |call| {
        let channel = channel.clone();
        std::thread::spawn(move || calls::generate(call, channel));
    });

    host.handle("settings.get", |call| {
        std::thread::spawn(move || {
            // `GENUI_REPLAY` (development) answers without DeepSeek, so it needs no key.
            let replay = std::env::var_os("GENUI_REPLAY").map(|_| "replay".to_owned());
            let state = keychain::get().map(|key| key.or(replay));
            dev::event("key", json!({ "hasKey": matches!(state, Ok(Some(_))), "error": state.as_ref().err() }));
            call.reply(state.map(|key| {
                json!({
                    "hasKey": key.is_some(),
                    "hint": key.as_deref().map(keychain::hint),
                    "models": deepseek::MODEL_IDS,
                })
            }))
        });
    });

    // Checks the key with DeepSeek before keeping it, so a typo shows at once.
    host.handle("settings.setKey", |call| {
        std::thread::spawn(move || {
            let key = call.data()["key"].as_str().unwrap_or("").trim().to_owned();
            let result = if key.is_empty() {
                Err("key 是空的".to_owned())
            } else {
                deepseek::Client::new(key.clone(), "")
                    .verify()
                    .and_then(|()| keychain::set(&key))
                    .map(|()| json!({ "hint": keychain::hint(&key) }))
            };
            call.reply(result);
        });
    });

    host.handle("settings.clearKey", |call| {
        std::thread::spawn(move || call.reply(keychain::delete().map(|()| Value::Null)));
    });
}
