//! Runs a compiled module as the app's sandbox runs it (its capabilities, the embedding limits, a desktop window
//! of the sandbox's size), but on a headless NSH engine: no window. It renders in dark and in light mode, its texts
//! must be readable on what is behind them, and each enabled button is pressed in turn; what fails on the way is
//! reported. The model's `check` tool (`generate.rs`) and the tests run it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use std::collections::HashMap;

use nsh_core::geometry::{Points, Size};
use nsh_core::platform::{ColorScheme, Os};
use nsh_core::{JsonValue, MountItem, NodeId};
use nsh_engine::{Bundle, EngineConfig, HostChannel, Limits};
use nsh_testing::Headless;

/// What generated code may do, here and in the app's sandbox (`src/sandbox.rs`): post `error` to the host.
/// No network, files, clipboard, ...
pub const CAPABILITIES: &str = r#"{ "host": ["error"] }"#;
/// Buttons pressed at most, one after another.
const PRESSES: usize = 8;

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Report {
    /// The texts on screen after the first render.
    pub texts: Vec<String>,
    /// The buttons found, and the enabled ones pressed (their selectors).
    pub buttons: usize,
    pub pressed: Vec<String>,
    /// Errors: thrown and not caught, or caught by the host's ErrorBoundary and posted.
    pub errors: Vec<String>,
}

/// Contrast below this (WCAG's ratio; 3 is its minimum for large text) is an error: the text cannot be read.
const MIN_CONTRAST: f64 = 3.0;
/// Unreadable texts reported at most.
const UNREADABLE: usize = 12;

/// Runs the module in dark mode (texts, buttons and presses), then in light mode (only its first render); the
/// errors of both, the light ones marked.
pub fn render(js: &str) -> Report {
    let mut report = render_in(js, ColorScheme::Dark, true);
    if report.errors.is_empty() {
        let light = render_in(js, ColorScheme::Light, false);
        report.errors.extend(light.errors.into_iter().map(|e| format!("in light mode: {e}")));
    } else {
        report.errors = report.errors.into_iter().map(|e| format!("in dark mode: {e}")).collect();
    }
    report
}

fn render_in(js: &str, scheme: ColorScheme, press: bool) -> Report {
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
    config.environment.color_scheme = scheme;
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
    let views = Arc::new(Mutex::new(Views::default()));
    let model = views.clone();
    app.observe(move |_, batch| model.lock().unwrap().apply(&batch.items));
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
    report.errors.extend(views.lock().unwrap().unreadable(scheme));
    if !press || !report.errors.is_empty() {
        return report;
    }

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


/// The views as the batches built them: parents, backgrounds, texts' spans. Enough to tell what color is behind
/// a text, which the headless screen does not say.
#[derive(Default)]
struct Views {
    parent: HashMap<NodeId, NodeId>,
    background: HashMap<NodeId, u32>,
    /// A text view's spans: the text and its color.
    spans: HashMap<NodeId, Vec<(String, u32)>>,
}

impl Views {
    fn apply(&mut self, items: &[MountItem]) {
        for item in items {
            match item {
                MountItem::Insert { parent, child, .. } => {
                    self.parent.insert(*child, *parent);
                }
                MountItem::Remove { id } => {
                    self.parent.remove(id);
                }
                MountItem::Delete { id } => {
                    self.parent.remove(id);
                    self.background.remove(id);
                    self.spans.remove(id);
                }
                MountItem::ViewStyle { id, style } => {
                    self.background.insert(*id, style.background_color.0);
                }
                MountItem::Text { id, text, .. } => {
                    let chars: Vec<char> = text.text.chars().collect();
                    let spans = text
                        .spans
                        .iter()
                        .map(|s| {
                            let part: String = chars
                                .get(s.start as usize..(s.end as usize).min(chars.len()))
                                .map(|c| c.iter().collect())
                                .unwrap_or_default();
                            (part, s.style.color.0)
                        })
                        .collect();
                    self.spans.insert(*id, spans);
                }
                _ => {}
            }
        }
    }

    /// The color behind `id`: its own and its ancestors' backgrounds composited, over the surface's.
    fn behind(&self, id: NodeId, scheme: ColorScheme) -> [f64; 3] {
        let mut layers = Vec::new();
        let mut at = Some(id);
        while let Some(node) = at {
            if let Some(&bg) = self.background.get(&node) {
                let alpha = (bg & 0xff) as f64 / 255.0;
                if alpha > 0.0 {
                    layers.push((rgb(bg), alpha));
                    if alpha >= 0.99 {
                        break;
                    }
                }
            }
            at = self.parent.get(&node).copied();
        }
        // What shows where the app paints nothing: the theme's background (shadcn's neutral).
        let mut color = match scheme {
            ColorScheme::Dark => rgb(0x0a0a0aff),
            ColorScheme::Light => rgb(0xffffffff),
        };
        for (layer, alpha) in layers.into_iter().rev() {
            for c in 0..3 {
                color[c] = layer[c] * alpha + color[c] * (1.0 - alpha);
            }
        }
        color
    }

    /// The texts whose color is too close to what is behind them, as errors.
    fn unreadable(&self, scheme: ColorScheme) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids: Vec<&NodeId> = self.spans.keys().collect();
        ids.sort();
        for id in ids {
            let back = self.behind(*id, scheme);
            for (text, color) in &self.spans[id] {
                let text = text.trim();
                if text.is_empty() || (color & 0xff) < 0x20 {
                    continue;
                }
                let ratio = contrast(rgb(*color), back);
                if ratio < MIN_CONTRAST {
                    out.push(format!(
                        "text {:?} cannot be read: {} on {} (contrast {ratio:.1}); give it a text color class that \
                         matches its background (`text-foreground`, `text-muted-foreground`, `text-card-foreground`, \
                         `text-primary-foreground` on `bg-primary`, ...)",
                        text.chars().take(40).collect::<String>(),
                        hex(rgb(*color)),
                        hex(back),
                    ));
                }
            }
            if out.len() >= UNREADABLE {
                out.push("(more texts cannot be read)".into());
                break;
            }
        }
        out
    }
}

fn rgb(c: u32) -> [f64; 3] {
    [((c >> 24) & 0xff) as f64 / 255.0, ((c >> 16) & 0xff) as f64 / 255.0, ((c >> 8) & 0xff) as f64 / 255.0]
}

fn hex(c: [f64; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", (c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8)
}

/// WCAG's contrast ratio of two colors.
fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let lum = |c: [f64; 3]| {
        let ch = |v: f64| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])
    };
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}
