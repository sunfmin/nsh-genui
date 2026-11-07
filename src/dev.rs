//! Development controls, on with `GENUI_DEV=1`: commands read from stdin, one per line, so the app can be
//! driven and looked at where nobody sees its screen (a remote Mac, CI).
//!
//! - `shot <path>`: a PNG of the window, drawn in process (`cacheDisplayInRect:`, as NSH's test agent does
//!   where the window server gives no picture); needs no Screen Recording permission.
//! - `keychain-check`: writes, reads and removes a keychain item of its own.
//! - anything else is sent to the shell as the host event `dev` (`{ "line": … }`), which `shell/App.tsx`
//!   handles: `prompt <text>`, `generate new|edit|fix`, `settings open|close`, `tab preview|code`.
//!
//! Each command prints `ok <line>` or `err <line>: <why>` when done.

use std::io::BufRead;

use nsh_engine::HostChannel;
use nsh_platform_macos::{
    objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject},
    objc2_app_kit::{NSApplication, NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey, NSWindow},
    objc2_foundation::NSDictionary,
};
use serde_json::json;

pub fn start(host: HostChannel) {
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            let line = line.trim().to_owned();
            if line.is_empty() {
                continue;
            }
            if line == "keychain-check" {
                report(&line, crate::keychain::check());
            } else if let Some(path) = line.strip_prefix("shot ") {
                let path = path.trim().to_owned();
                let (tx, rx) = std::sync::mpsc::channel();
                dispatch2::DispatchQueue::main().exec_async(move || {
                    let _ = tx.send(shot().and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string())));
                });
                report(&line, rx.recv().unwrap_or_else(|_| Err("the main queue dropped it".into())));
            } else {
                report(&line, host.send("dev", json!({ "line": line })));
            }
        }
    });
}

fn report(line: &str, result: Result<(), String>) {
    match result {
        Ok(()) => println!("ok {line}"),
        Err(e) => println!("err {line}: {e}"),
    }
}

/// The key window's content, else the first window's, as a PNG.
fn shot() -> Result<Vec<u8>, String> {
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let window: Retained<NSWindow> =
        app.keyWindow().or_else(|| app.windows().firstObject()).ok_or("the app has no window")?;
    let view = window.contentView().ok_or("the window has no content")?;
    let bounds = view.bounds();
    let rep = view.bitmapImageRepForCachingDisplayInRect(bounds).ok_or("cannot make a bitmap of the window")?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    png(&rep)
}

fn png(rep: &NSBitmapImageRep) -> Result<Vec<u8>, String> {
    let properties = NSDictionary::<NSBitmapImageRepPropertyKey, AnyObject>::new();
    // SAFETY: an empty property dictionary is valid for PNG.
    let data = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties) };
    Ok(data.ok_or("cannot encode the picture")?.to_vec())
}
