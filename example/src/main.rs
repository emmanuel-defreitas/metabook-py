//! Metabook — GPUI desktop example client for the Book Structure API.
//!
//! Run the API first (`make dev` in the repository root), then `cargo run`.
//! Point at another instance with `METABOOK_API=https://… cargo run`.

mod api;
mod app;
mod theme;

use std::borrow::Cow;

use ely_gpui_component::theme::ActiveTheme as _;
use gpui::{
    px, size, App, AppContext as _, AssetSource, Result, SharedString, TitlebarOptions,
    WindowOptions,
};
use gpui_component::Root;

use crate::app::MetabookApp;

const CUSTOM_ICON: &str = "icons/document-magnifying-glass.svg";

/// This app's own icons, then Ely's, then gpui-component's. GPUI takes one
/// asset source, and Ely's `init` panics unless `icons/check.svg` loads
/// through it, so anything we don't carry falls through.
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == CUSTOM_ICON {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/document-magnifying-glass.svg"
            ))));
        }
        if let Some(asset) = ely_gpui_component::Assets.load(path)? {
            return Ok(Some(asset));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = ely_gpui_component::Assets.list(path)?;
        assets.extend(gpui_component_assets::Assets.list(path)?);
        if CUSTOM_ICON.starts_with(path) {
            assets.push(CUSTOM_ICON.into());
        }
        Ok(assets)
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            ely_gpui_component::init(cx).expect("Ely failed to start");
            theme::init(cx);
            // This single-window bundle must exit so its launcher can stop the
            // owned API and Finder can launch a fresh window next time.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let traffic_light_origin = cx.theme().traffic_light_origin();

            cx.spawn(async move |cx| {
                // No native title bar: Ely's TitleBar owns dragging and
                // double-click zoom. AppKit must not treat the bar as a system
                // move region, or macOS handles the double-click itself and
                // delays clicks while it disambiguates. The traffic lights sit
                // where Ely leaves room for them.
                let options = WindowOptions {
                    window_min_size: Some(size(px(960.), px(640.))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,

                        traffic_light_position: Some(traffic_light_origin),
                        title: None,
                    }),
                    app_owns_titlebar_drag: true,
                    ..Default::default()
                };
                cx.open_window(options, |window, cx| {
                    let app = cx.new(|cx| MetabookApp::new(window, cx));
                    cx.new(|cx| Root::new(app, window, cx))
                })
                .expect("failed to open window");
            })
            .detach();
        });
}
