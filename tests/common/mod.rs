//! What the tests share: the app's own headless check, and saved replies.

pub use nsh_genui::verify::render;

#[allow(dead_code, reason = "tests/harness.rs uses it, tests/live.rs does not")]
/// A module from a saved reply (`examples/`), compiled as the app compiles it.
pub fn replay_js(file: &str) -> String {
    let reply = std::fs::read_to_string(format!("{}/examples/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    nsh_genui::generate::replay(&reply).unwrap().js
}
