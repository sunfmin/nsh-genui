//! The native component `genui.sandbox`: an [`AppView`] of its own inside the shell, so generated code runs
//! on a separate engine and QuickJS VM, with the embedding limits (256 MB heap, 2 s per call, 200 000 nodes)
//! and no capabilities but reporting its errors to the host.

use std::rc::Rc;

use nsh_core::JsonValue;
use nsh_platform_macos::{
    AppOptions, AppView, Bundle, ComponentContext, Components, NativeComponent,
    objc2::{MainThreadMarker, rc::Retained},
    objc2_app_kit::NSView,
};

use nsh_genui::verify::CAPABILITIES;

struct Sandbox {
    app: Rc<AppView>,
    /// The id of the program running, so a prop sent again does not reload it.
    loaded: Option<u64>,
}

impl NativeComponent for Sandbox {
    fn view(&self) -> Retained<NSView> {
        self.app.view()
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        if name != "program" {
            return;
        }
        let Some(program) = value.filter(|v| v.is_object()) else {
            self.app.stop();
            self.loaded = None;
            return;
        };
        let id = program["id"].as_u64();
        if id == self.loaded {
            return;
        }
        self.loaded = id;
        let js = program["js"].as_str().unwrap_or_default().to_owned();
        match Bundle::source(js).with_capabilities_json(Some(CAPABILITIES)) {
            Ok(bundle) => {
                self.app.load(bundle.into());
                crate::dev::event("loaded", serde_json::json!({ "id": id }));
            }
            Err(e) => log::error!(target: "genui", "sandbox capabilities: {e}"),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.app.stop();
    }
}

pub fn register(components: &Components) {
    components.register(
        "genui.sandbox",
        Rc::new(|context: ComponentContext| -> Box<dyn NativeComponent> {
            let mtm = MainThreadMarker::new().expect("native components are made on the main thread");
            // `AppOptions::default()`: the embedding limits (`Limits::default`), not a standalone app's.
            let app = AppView::new(mtm, AppOptions::default());
            app.host().handle("error", move |call| {
                crate::dev::event("sandbox-error", call.data().clone());
                context.emit("error", call.data().clone());
                call.resolve(JsonValue::Null);
            });
            Box::new(Sandbox { app, loaded: None })
        }),
    );
}
