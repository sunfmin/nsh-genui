//! Checks a module as the model's `check` tool does: `cargo run --release --example check -- <file.tsx>...`.
//! Prints each file's result; exits 1 when one fails.

fn main() {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        let tsx = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let checked = nsh_genui::generate::check(&tsx);
        let r = &checked.report;
        println!("{} {path}: {} texts, {} buttons, {} pressed", if checked.ok() { "PASS" } else { "FAIL" }, r.texts.len(), r.buttons, r.pressed.len());
        for e in checked.errors() {
            println!("    {}", e.lines().next().unwrap_or(""));
        }
        failed |= !checked.ok();
    }
    std::process::exit(failed as i32);
}
