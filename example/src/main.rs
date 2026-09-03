//! Metabook — GPUI desktop example client for the Book Structure API.
//!
//! Run the API first (`make dev` in the repository root), then `cargo run`.
//! Point at another instance with `METABOOK_API=https://… cargo run`.

mod api;
mod app;

use std::borrow::Cow;

use gpui::{
    point, px, size, App, AppContext as _, AssetSource, Result, SharedString, TitlebarOptions,
    WindowOptions,
};
use gpui_component::{Root, Theme, TitleBar};

use crate::app::MetabookApp;

/// The component library's embedded icons, plus this app's own. Custom SVGs
/// live under `assets/` and are compiled in; anything we don't carry falls
/// through to `gpui_component_assets`.
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icons/document-magnifying-glass.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/document-magnifying-glass.svg"
            )))),
            _ => gpui_component_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);

            // Rounder controls app-wide: buttons, inputs, selects, and cards
            // all read the theme radius, so one token bump rounds the whole
            // system instead of per-call-site overrides. `radius` is the
            // rounded-xl step (12px) and `radius_lg` keeps dialogs and
            // notifications one step rounder. This survives light/dark
            // switches because the default theme configs set no radius; the
            // sync pushes the new radius down to Base-owned scrollbars.
            {
                let theme = Theme::global_mut(cx);
                theme.radius = px(14.);
                theme.radius_lg = px(18.);
            }
            Theme::sync_base(cx);

            cx.spawn(async move |cx| {
                // No native title bar: the in-app TitleBar owns dragging,
                // double-click zoom, and the traffic-light inset.
                let title_bar_options = Option::Some(TitlebarOptions {
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(24.), px(24.))),
                    title: None,
                });
                let options = WindowOptions {
                    window_min_size: Some(size(px(560.), px(440.))),
                    titlebar: title_bar_options,
                    ..TitleBar::window_options()
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
