//! Metabook — GPUI desktop example client for the Book Structure API.
//!
//! Run the API first (`make dev` in the repository root), then `cargo run`.
//! Point at another instance with `METABOOK_API=https://… cargo run`.

mod api;
mod app;

use gpui::{App, AppContext as _, WindowOptions, px, size};
use gpui_component::{Root, TitleBar};
use gpui_navigator::{Route, init_router, navigate};

use crate::app::MetabookApp;

fn main() {
    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);

            cx.spawn(async move |cx| {
                // No native title bar: the in-app TitleBar owns dragging,
                // double-click zoom, and the traffic-light inset.
                let options = WindowOptions {
                    window_min_size: Some(size(px(560.), px(440.))),
                    ..TitleBar::window_options()
                };
                cx.open_window(options, |window, cx| {
                    let app = cx.new(|cx| MetabookApp::new(window, cx));

                    // The search and upload forms are routes; the outlet in
                    // MetabookApp::render animates transitions between them.
                    // Pages read a dedicated flags entity, never the app
                    // entity itself (which is mid-render when the outlet runs).
                    let handles = app.update(cx, |this, cx| this.form_handles(cx));
                    let search_handles = handles.clone();
                    let upload_handles = handles;
                    init_router(cx, move |router| {
                        router.add_route(Route::new("/", move |_, cx, _| {
                            MetabookApp::search_form(&search_handles, cx)
                        }));
                        router.add_route(Route::new("/upload", move |_, cx, _| {
                            MetabookApp::upload_form(&upload_handles, cx)
                        }));
                    });
                    navigate(cx, "/");

                    cx.new(|cx| Root::new(app, window, cx))
                })
                .expect("failed to open window");
            })
            .detach();
        });
}
