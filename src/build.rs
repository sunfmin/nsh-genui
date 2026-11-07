//! From a model's reply to the JS a sandbox runs: the module taken out of the reply, mounted by the host
//! under an error boundary, compiled by NSH's compiler, and its imports checked against what NSH serves.
//!
//! This is the "server checks" step of NSH's flow for generated UI; it runs in the app for the demo.

use std::sync::LazyLock;

use nsh_core::element::description::SetDescription;
use regex::Regex;

/// The UI sets the engines serve (NSH's `nsh/base`, the default).
fn ui_sets() -> Vec<SetDescription> {
    vec![nsh_base::registry().into()]
}

/// The modules generated code may import (`nsh:rt` is what compiled JSX imports).
const ALLOWED: [&str; 4] = ["nsh", "nsh/base", "nsh/base/plot", "nsh:rt"];

/// Mounts the module's `App` under an error boundary that reports to the host (`src/sandbox.rs`).
const FOOTER: &str = r#"
import { mount as genuiMount, ErrorBoundary as GenuiBoundary, Text as GenuiText, View as GenuiView, host as genuiHost } from "nsh";
genuiMount(() => (
  <GenuiView class="flex-1">
    <GenuiBoundary
      fallback={(error: any) => {
        genuiHost.post("error", { message: String(error?.stack ? `${error.message ?? error}\n${error.stack}` : error) });
        return (
          <GenuiView class="flex-1 items-center justify-center p-6">
            <GenuiText class="text-destructive">{String(error?.message ?? error)}</GenuiText>
          </GenuiView>
        );
      }}
    >
      <GenuiApp />
    </GenuiBoundary>
  </GenuiView>
));
"#;

/// The last ```tsx block of a reply, and the sentence before it.
pub fn extract(reply: &str) -> Result<(String, String), String> {
    static BLOCK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)```(?:tsx|typescript|ts|jsx|javascript|js)?[ \t]*\n(.*?)\n```").unwrap());
    let found = BLOCK
        .captures_iter(reply)
        .last()
        .ok_or("the reply has no complete ```tsx code block (cut off, or not code)")?;
    let start = found.get(0).unwrap().start();
    let summary = reply[..start].trim().lines().last().unwrap_or("").trim().to_owned();
    Ok((found[1].to_owned(), summary))
}

/// The module's source made into a whole app: its default export renamed to `GenuiApp`, a `mount` of its own
/// removed, the footer added.
fn wrap(tsx: &str) -> Result<String, String> {
    static MOUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*mount\([^\n]*\);?\s*$").unwrap());
    static DEFAULT_FN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"export\s+default\s+function(\s+[A-Za-z_$][\w$]*)?\s*\(").unwrap());
    static DEFAULT_NAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^\s*export\s+default\s+([A-Za-z_$][\w$]*)\s*;?\s*$").unwrap());

    let code = MOUNT.replace_all(tsx, "");
    let code = if DEFAULT_FN.is_match(&code) {
        DEFAULT_FN.replace(&code, "function GenuiApp(").into_owned()
    } else if let Some(c) = DEFAULT_NAME.captures(&code) {
        let name = c[1].to_owned();
        DEFAULT_NAME.replace(&code, format!("const GenuiApp = {name};")).into_owned()
    } else {
        return Err("the module has no `export default function App()`".into());
    };
    Ok(code + FOOTER)
}

/// Compiles a generated module to the JS a sandbox runs.
pub fn generated(tsx: &str) -> Result<String, String> {
    let js = nsh_bundler::compile_module("App.tsx", &wrap(tsx)?, &ui_sets())
        .map_err(|e| format!("compile error:\n{e}"))?;
    check_imports(&js)?;
    Ok(js)
}

/// Compiles the shell (`shell/App.tsx`).
pub fn shell(tsx: &str) -> Result<String, String> {
    let js = nsh_bundler::compile_module("App.tsx", tsx, &ui_sets())?;
    check_imports(&js)?;
    Ok(js)
}

/// Checks the compiled module's imports: only NSH's modules, only names they export. The compiler has removed
/// imports only types use, so what is left is what runs. (`nsh build` does this when bundling; the one-module
/// compiler does not.)
fn check_imports(js: &str) -> Result<(), String> {
    static IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"import\s*([^;]*?)\s*from\s*["']([^"']+)["']"#).unwrap());
    static SIDE_EFFECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?m)^\s*import\s*["']([^"']+)["']"#).unwrap());
    let sets = ui_sets();
    let mut problems = Vec::new();
    for c in SIDE_EFFECT.captures_iter(js) {
        problems.push(format!("`import \"{}\"`: only imports of names from NSH's modules work", &c[1]));
    }
    for c in IMPORT.captures_iter(js) {
        let (clause, spec) = (c[1].trim(), &c[2]);
        if !ALLOWED.contains(&spec) {
            problems.push(format!("`{spec}` cannot be imported: only \"nsh\", \"nsh/base\" and \"nsh/base/plot\" exist here"));
            continue;
        }
        if spec == "nsh:rt" {
            continue;
        }
        let Some(open) = clause.find('{') else {
            if !clause.starts_with('*') {
                problems.push(format!("`{spec}` has no default export: import names, `import {{ View }} from \"{spec}\"`"));
            }
            continue;
        };
        let exports = nsh_js::native_module_exports(spec, &sets).unwrap_or_default();
        let names = clause[open + 1..].trim_end_matches('}');
        for name in names.split(',').map(|n| n.split(" as ").next().unwrap_or("").trim()) {
            if !name.is_empty() && !exports.iter().any(|e| e == name) {
                problems.push(format!("`{name}` is not exported by \"{spec}\""));
            }
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems.join("\n")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
import { signal, Text, View } from "nsh";
import { Button } from "nsh/base";
export default function App() {
  const n = signal(0);
  return <View class="flex-1"><Button onPress={() => n.update((x) => x + 1)}><Text>{n()}</Text></Button></View>;
}
"#;

    #[test]
    fn a_good_module_compiles_and_is_mounted_once() {
        let js = generated(GOOD).unwrap();
        assert!(js.contains("function GenuiApp("), "{js}");
        assert_eq!(js.matches("genuiMount(").count(), 1, "{js}");
    }

    #[test]
    fn a_mount_of_its_own_is_removed() {
        let js = generated(&format!("{GOOD}\nmount(App);\n")).unwrap();
        assert!(!js.contains("mount(App)"), "{js}");
    }

    #[test]
    fn a_missing_export_is_refused() {
        let err = generated(&GOOD.replace("signal,", "useState, signal,").replace("const n = signal(0);", "const n = signal(0); useState(0);")).unwrap_err();
        assert!(err.contains("`useState` is not exported by \"nsh\""), "{err}");
    }

    #[test]
    fn a_package_import_is_refused() {
        let err = generated(&format!("import React from \"react\";\nconsole.log(React);\n{GOOD}")).unwrap_err();
        assert!(err.contains("`react` cannot be imported"), "{err}");
    }

    #[test]
    fn a_syntax_error_is_reported() {
        let err = generated(&GOOD.replace("</View>;", "</Vie>;")).unwrap_err();
        assert!(err.starts_with("compile error"), "{err}");
    }

    #[test]
    fn the_code_block_and_summary_are_taken_from_the_reply() {
        let (code, summary) = extract("好的。\n一个计数器。\n```tsx\nexport default function App() {}\n```\n").unwrap();
        assert_eq!(code, "export default function App() {}");
        assert_eq!(summary, "一个计数器。");
        assert!(extract("```tsx\ncut off").is_err());
    }
}
