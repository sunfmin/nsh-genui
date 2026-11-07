//! The headless harness itself, on saved replies (no network): a working module passes, a broken one is caught.

mod common;

#[test]
fn a_working_module_renders_and_its_buttons_press() {
    let r = common::render(&common::replay_js("replay-pomodoro.md"));
    // The pomodoro's third button throws on purpose, and the boundary posts it.
    assert!(r.texts.iter().any(|t| t.contains("25:00")), "{r:#?}");
    assert_eq!(r.buttons, 3, "{r:#?}");
    assert_eq!(r.errors.len(), 1, "{r:#?}");
    assert!(r.errors[0].contains("故意的运行时错误"), "{r:#?}");
}

#[test]
fn a_module_that_throws_while_rendering_is_caught() {
    let r = common::render(&common::replay_js("replay-broken.md"));
    assert!(r.errors.iter().any(|e| e.contains("profile")), "{r:#?}");
}
