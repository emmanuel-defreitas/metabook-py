//! Router page components.
//!
//! These forms are built outside the app entity's render pass, so they read
//! state through `FormHandles` and update the app only from event handlers.

use gpui::{div, px, AnyElement, App, IntoElement, ParentElement as _, SharedString, Styled as _};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::select::Select;
use gpui_component::{h_flex, ActiveTheme as _, Disableable as _, Icon, IconName};

use super::styles::{DETAIL_FIELD_WIDTH, ISBN_FIELD_WIDTH};
use super::{FormHandles, MetabookApp};

impl MetabookApp {
    pub(crate) fn search_form(handles: &FormHandles, cx: &mut App) -> AnyElement {
        let processing = handles.flags.read(cx).processing;
        let (query, isbn, tokenizer, detail) = (
            handles.query.clone(),
            handles.isbn.clone(),
            handles.tokenizer.clone(),
            handles.detail.clone(),
        );

        h_flex()
            .gap_2()
            .items_center()
            .child(div().flex_1().child(Input::new(&query)))
            .child(div().w(px(ISBN_FIELD_WIDTH)).child(Input::new(&isbn)))
            .child(div().w_56().child(Select::new(&tokenizer)))
            .child(div().w(px(DETAIL_FIELD_WIDTH)).child(Select::new(&detail)))
            .child(
                Button::new("search")
                    .primary()
                    .icon(IconName::Search)
                    .label("Search")
                    .loading(processing)
                    .disabled(processing)
                    .on_click({
                        let app = handles.app.clone();
                        move |_, window, cx| {
                            app.update(cx, |this, cx| this.start_search(window, cx))
                                .ok();
                        }
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn upload_form(handles: &FormHandles, cx: &mut App) -> AnyElement {
        let (processing, epub_name) = {
            let flags = handles.flags.read(cx);
            (flags.processing, flags.epub_name.clone())
        };
        let has_file = epub_name.is_some();
        let chosen: SharedString = epub_name.unwrap_or_else(|| "No file chosen".into());

        h_flex()
            .gap_2()
            .items_center()
            .child(
                Button::new("choose-epub")
                    .label("Choose EPUB…")
                    .disabled(processing)
                    .on_click({
                        let app = handles.app.clone();
                        move |_, _, cx| {
                            app.update(cx, |this, cx| this.choose_epub(cx)).ok();
                        }
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(chosen),
            )
            .child(div().w_56().child(Select::new(&handles.tokenizer)))
            .child(
                div()
                    .w(px(DETAIL_FIELD_WIDTH))
                    .child(Select::new(&handles.detail)),
            )
            .child(
                Button::new("analyze")
                    .primary()
                    .icon(Icon::default().path("icons/document-magnifying-glass.svg"))
                    .label("Analyze")
                    .loading(processing)
                    .disabled(processing || !has_file)
                    .on_click({
                        let app = handles.app.clone();
                        move |_, window, cx| {
                            app.update(cx, |this, cx| this.start_upload(window, cx))
                                .ok();
                        }
                    }),
            )
            .into_any_element()
    }
}
