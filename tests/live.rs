//! Integration: real DeepSeek generations, run on a headless NSH engine as the app's sandbox runs them.
//!
//! Each case lets DeepSeek write the UI the shell's examples describe, checking it with its tools until it passes (the first also changes its result, as
//! "在当前版本上改" does), then checks independently that the accepted module renders, shows what was asked for, and survives pressing its
//! buttons. Costs tokens, so ignored by default: `scripts/integration.sh` runs them with the key from mytokens.
//! `GENUI_TEST_MODEL` and `GENUI_TEST_THINKING` (off, low, default) pick the model and its reasoning. Each case's module and numbers are written
//! to `target/live/<model>-<thinking>/`.

mod common;

use std::sync::LazyLock;

use nsh_genui::deepseek::{Client, MODEL_IDS, Thinking};
use nsh_genui::generate::{Generation, Request, generate};
use serde_json::json;

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    let key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY: run scripts/integration.sh");
    let model = std::env::var("GENUI_TEST_MODEL").unwrap_or_else(|_| MODEL_IDS[0].to_owned());
    let mut client = Client::new(key, &model);
    if let Ok(t) = std::env::var("GENUI_TEST_THINKING") {
        client.thinking = Thinking::from_id(&t).expect("GENUI_TEST_THINKING: off, low or default");
    }
    client
});

/// Generates, runs and checks one case; returns the generation for a follow-up.
fn case(name: &str, request: Request, expect: &[&str]) -> Generation {
    let g = generate(&CLIENT, &request, &mut |_| {}).unwrap_or_else(|e| panic!("[{name}] generation failed: {e}"));
    let report = common::render(&g.js);

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/live")
        .join(format!("{}-{}", CLIENT.model, CLIENT.thinking.id()));
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join(format!("{name}.tsx")), &g.tsx).unwrap();
    let summary = json!({
        "case": name, "instruction": request.instruction, "model": g.model, "thinking": CLIENT.thinking.id(),
        "seconds": g.seconds, "turns": g.turns, "checks": g.checks, "failed": g.failed,
        "promptTokens": g.prompt_tokens, "completionTokens": g.completion_tokens, "summary": g.summary,
        "texts": report.texts, "buttons": report.buttons, "pressed": report.pressed, "errors": report.errors,
    });
    std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    eprintln!(
        "[{name}] {}/{} {:.1}s, {} turns, {} checks ({} failed), {} tokens out; {} texts, {} buttons, {} pressed, {} errors",
        g.model, CLIENT.thinking.id(), g.seconds, g.turns, g.checks, g.failed.len(), g.completion_tokens, report.texts.len(), report.buttons, report.pressed.len(),
        report.errors.len()
    );

    assert!(report.errors.is_empty(), "[{name}] errors: {:#?}\n{}", report.errors, g.tsx);
    assert!(report.texts.len() >= 3, "[{name}] shows almost nothing: {:?}", report.texts);
    let all = report.texts.join("\n");
    for want in expect {
        assert!(all.contains(want), "[{name}] does not show {want:?}; it shows:\n{all}");
    }
    g
}

fn ask(instruction: &str) -> Request {
    Request { instruction: instruction.into(), ..Request::default() }
}

#[test]
#[ignore]
fn pomodoro_then_a_change_to_it() {
    let g = case("pomodoro", ask("番茄钟：开始/暂停/重置，显示今天完成了几个"), &["25:00"]);
    let edited = case(
        "pomodoro-edit",
        Request {
            instruction: "加一个设置：可以把专注时长改成 15、25 或 50 分钟".into(),
            base: Some(g.tsx),
            error: None,
        },
        &[],
    );
    // In the code, not on screen: the choice may sit in a menu that is closed.
    for want in ["15", "50"] {
        assert!(edited.tsx.contains(want), "[pomodoro-edit] has no {want}:\n{}", edited.tsx);
    }
}

#[test]
#[ignore]
fn ledger() {
    case("ledger", ask("记账本：添加收支、分类筛选，顶部显示本月结余"), &["结余"]);
}

#[test]
#[ignore]
fn currency() {
    case("currency", ask("汇率换算：人民币、美元、日元、欧元互换，可交换方向"), &["美元", "日元"]);
}

#[test]
#[ignore]
fn weather() {
    case("weather", ask("一周天气卡片，带温度折线图"), &["°"]);
}

#[test]
#[ignore]
fn kanban() {
    case("kanban", ask("待办看板：三列（待办/进行中/完成），可以把卡片往右移"), &["待办", "进行中", "完成"]);
}
