//! Runs a generated module as the app's sandbox runs it (its capabilities, the embedding limits, a desktop
//! window of the sandbox's size), but on a headless NSH engine: no window, no keychain, no clicks.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use nsh_core::JsonValue;
use nsh_core::geometry::{Points, Size};
use nsh_core::platform::Os;
use nsh_engine::{Bundle, EngineConfig, HostChannel, Limits};
use nsh_testing::Headless;

/// The sandbox's capabilities (`src/sandbox.rs`).
const CAPABILITIES: &str = r#"{ "host": ["error"] }"#;
/// Buttons pressed at most, one after another.
const PRESSES: usize = 8;

#[derive(Debug, Default)]
pub struct Report {
    /// The texts on screen after the first render.
    pub texts: Vec<String>,
    /// The buttons found, and the enabled ones pressed (their selectors).
    pub buttons: usize,
    pub pressed: Vec<String>,
    /// Errors: thrown and not caught, or caught by the host's ErrorBoundary and posted.
    pub errors: Vec<String>,
}

pub fn render(js: &str) -> Report {
    let posted = Arc::new(Mutex::new(Vec::<String>::new()));
    let host = HostChannel::default();
    let sink = posted.clone();
    host.handle("error", move |call| {
        sink.lock().unwrap().push(call.data()["message"].as_str().unwrap_or("").to_owned());
        call.resolve(JsonValue::Null);
    });

    let bundle = Bundle::source(js.to_owned()).with_capabilities_json(Some(CAPABILITIES)).unwrap();
    let mut config = EngineConfig::new(bundle);
    config.viewport = Size::new(Points(900.0), Points(760.0));
    config.environment.window = config.viewport;
    config.environment.os = Os::MacOs;
    config.limits = Limits::default();
    config.ui_sets = vec![nsh_base::registry()];
    config.host = host;

    let mut report = Report::default();
    let mut app = match Headless::start(config) {
        Ok(app) => app,
        Err(e) => {
            report.errors.push(format!("engine: {e}"));
            return report;
        }
    };
    let step = |report: &mut Report, what: &str, result: Result<(), String>| {
        if let Err(e) = result {
            report.errors.push(format!("{what}: {e}"));
        }
        report.errors.extend(posted.lock().unwrap().drain(..).map(|m| format!("{what}: {m}")));
        report.errors.is_empty()
    };

    let first = app.first_render().and_then(|()| app.idle(Duration::from_millis(300)).map(|_| ()));
    if !step(&mut report, "first render", first) {
        return report;
    }
    report.texts = app.screen().texts().into_iter().map(str::to_owned).collect();
    report.buttons = app.screen().count("role:button").unwrap_or(0);

    // Press the buttons in turn (as they are after the presses before), each then left to settle.
    for n in 0..report.buttons.min(PRESSES) {
        let selector = format!("role:button >> nth={n}");
        if !app.screen().matches(&selector).unwrap_or(false) {
            break;
        }
        // A disabled button (an "Add" while its field is empty) is no press a user can make.
        if !app.screen().enabled(&selector).unwrap_or(false) {
            continue;
        }
        let name = selector.clone();
        let pressed = app.press(&selector).and_then(|()| app.idle(Duration::from_millis(200)).map(|_| ()));
        report.pressed.push(name.clone());
        if !step(&mut report, &format!("press {name:?}"), pressed) {
            break;
        }
    }
    report
}

#[allow(dead_code, reason = "tests/harness.rs uses it, tests/live.rs does not")]
/// A module from a saved reply (`examples/`), compiled as the app compiles it.
pub fn replay_js(file: &str) -> String {
    let reply = std::fs::read_to_string(format!("{}/examples/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    nsh_genui::generate::replay(&reply).unwrap().js
}
