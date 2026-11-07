//! What turns a request into a module a sandbox runs: DeepSeek ([`deepseek`]), the checks ([`build`]) and the
//! attempts ([`generate`]). The app (`src/main.rs`) and the integration tests (`tests/live.rs`) share it.

pub mod build;
pub mod deepseek;
pub mod generate;
