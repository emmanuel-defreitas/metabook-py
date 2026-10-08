//! A single home canvas: identifiers, EPUB intake, and persisted recent results.
use super::{Cover, Library, MetabookApp, Phase};
use crate::api::LibraryBook;
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    feedback::Banner,
    files::{FileOperation, FileOperationProgress, FilePreview, TransferState},
    forms::{DropZone, SearchInput},
    layout::SimpleGrid,
    lists::DirEntry,
    motion::SkeletonCard,
    primitives::{FocusRing as _, Severity},
    theme::{ActiveTheme as _, ControlSize, Radius, TextSize},
};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, rems, svg, Animation, AnimationExt as _, AnyElement, Context, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled,
};
use gpui_component::{h_flex, v_flex};
use jiff::Timestamp;
use serde_json::Value;
use std::sync::Arc;

const WEEK: i64 = 7 * 24 * 60 * 60;
const BOOK_RATIO: f32 = 2.0 / 3.0;

/// FilePreview's default icon well is landscape. A cached SVG keeps both
/// loaded covers and missing-cover icons in the same portrait book shape.
pub(super) fn book_placeholder(cx: &gpui::App) -> (gpui::Hsla, Arc<gpui::Image>) {
    let color = cx.theme().colors.fg_muted;
    let rgb = color.to_rgb();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="300" viewBox="0 0 200 300"><g transform="translate(76 126) scale(2)" fill="none" stroke="rgb({:.0},{:.0},{:.0})" opacity="{}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 7v14m0-14C9 5 5 4 2 5v14c4-1 7 0 10 2m0-14c3-2 7-3 10-2v14c-4-1-7 0-10 2"/></g></svg>"#,
        rgb.r * 255.,
        rgb.g * 255.,
        rgb.b * 255.,
        color.a,
    );
    (
        color,
        Arc::new(gpui::Image::from_bytes(
            gpui::ImageFormat::Svg,
            svg.into_bytes(),
        )),
    )
}

/// Use recorded activity only; missing, malformed and future dates stay out.
fn recent_at(record: &Value, now: Timestamp) -> Option<Timestamp> {
    let latest = [
        &record["updated_at"],
        &record["created_at"],
        &record["scan"]["last_scanned_at"],
    ]
    .into_iter()
    .filter_map(|v| v.as_str()?.parse::<Timestamp>().ok())
    .max()?;
    (latest <= now && now.as_second() - latest.as_second() <= WEEK).then_some(latest)
}

impl MetabookApp {
    pub(super) fn render_home(&self, window: &gpui::Window, cx: &Context<Self>) -> AnyElement {
        let this = cx.entity();
        let drop = DropZone::new("home-epub-drop")
            .kinds(&["epub"])
            .hint("EPUB · Drop or browse to detect its structure")
            .on_drop(move |paths, window, cx| {
                if let Some(path) = paths.into_iter().next() {
                    this.update(cx, |this, cx| {
                        if this.is_processing() {
                            return;
                        }
                        this.set_epub_path(path, cx);
                        this.start_upload(window, cx);
                    });
                }
            });
        div()
            .id("home-page")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .when(self.is_processing(), |home| {
                        home.child(self.render_home_operation(cx))
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(rems(58.))
                            .px_8()
                            .py_16()
                            .gap_6()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        svg()
                                            .path("icons/metabook-mark.svg")
                                            .size(rems(2.))
                                            .text_color(cx.theme().colors.accent),
                                    )
                                    .child(
                                        div()
                                            .text_size(cx.theme().text_size(TextSize::Xxl))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("metaBook"),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_0()
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                div().flex_1().min_w_0().child(
                                                    SearchInput::new("home-search", &self.query)
                                                        .size(ControlSize::Lg),
                                                ),
                                            )
                                            .child(
                                                Button::new("home-search-submit", "Search")
                                                    .primary()
                                                    .size(ControlSize::Lg)
                                                    .loading(self.is_processing())
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            cx.stop_propagation();
                                                            this.start_search(window, cx);
                                                        },
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .opacity(if self.is_processing() { 0.55 } else { 1. })
                                            .child(drop),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_5()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(cx.theme().text_size(TextSize::Lg))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child("Recents"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(cx.theme().text_size(TextSize::Sm))
                                                    .text_color(cx.theme().colors.fg_muted)
                                                    .child("Past 7 days"),
                                            ),
                                    )
                                    .child(self.render_recent_previews(window, cx)),
                            ),
                    ),
            )
            .with_animation(
                ("home-entry", self.request_ix),
                Animation::new(ely_gpui_component::motion::duration(
                    ely_gpui_component::motion::BASE,
                    cx,
                ))
                .with_easing(ely_gpui_component::motion::ease_out_cubic),
                |home, t| home.opacity(t),
            )
            .into_any_element()
    }

    fn render_home_operation(&self, cx: &Context<Self>) -> AnyElement {
        let Phase::Processing { message, .. } = &self.phase else {
            return div().into_any_element();
        };
        let upload = self.upload_progress.as_ref().map(|p| p.snapshot());
        let uploaded = upload.is_some_and(|(sent, total)| total > 0 && sent == total);
        let message: SharedString = if uploaded {
            format!(
                "Parsing {} · waiting for the structural schema",
                self.epub_display_name().as_deref().unwrap_or("the EPUB")
            )
            .into()
        } else {
            message.clone()
        };
        v_flex()
            .w_full()
            .bg(cx.theme().colors.info_subtle)
            .child(Banner::new("home-parsing-banner", Severity::Info, message))
            .when_some(upload, |operation, (sent, total)| {
                operation.child(
                    div()
                        .w_full()
                        .max_w(rems(58.))
                        .mx_auto()
                        .px_8()
                        .py_3()
                        .child(
                            FileOperationProgress::new(
                                "home-upload-progress",
                                FileOperation::Copy,
                                1,
                                "Metabook",
                                (sent, total),
                                if uploaded {
                                    TransferState::Done
                                } else if total == 0 {
                                    TransferState::Queued
                                } else {
                                    TransferState::Moving { rate: 0 }
                                },
                            )
                            .current(self.epub_display_name().unwrap_or_else(|| "EPUB".into())),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_recent_previews(&self, window: &gpui::Window, cx: &Context<Self>) -> AnyElement {
        match &self.library {
            Library::Loading => v_flex()
                .gap_4()
                .children((0..2_usize).map(|ix| SkeletonCard::new(("recent-loading", ix))))
                .into_any_element(),
            Library::Failed(message) => v_flex()
                .gap_3()
                .child(
                    div()
                        .text_color(cx.theme().colors.fg_muted)
                        .child(message.clone()),
                )
                .child(
                    Button::new("home-reload", "Reload recents")
                        .variant(ButtonVariant::Outline)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_library(cx))),
                )
                .into_any_element(),
            Library::Ready(books) => {
                let now = Timestamp::now();
                let mut books: Vec<_> = books
                    .iter()
                    .filter_map(|book| Some((book, recent_at(&book.record, now)?)))
                    .collect();
                books.sort_by_key(|(_, date)| std::cmp::Reverse(*date));
                if books.is_empty() {
                    return div()
                        .py_8()
                        .text_color(cx.theme().colors.fg_muted)
                        .child("No books from the past week. Search or drop an EPUB to begin.")
                        .into_any_element();
                }
                SimpleGrid::new(
                    "recent-books-grid",
                    window.rem_size() * 15.,
                    window.rem_size() * 1.5,
                )
                .children(
                    books
                        .into_iter()
                        .map(|(book, date)| self.render_recent_preview(book, date, cx)),
                )
                .into_any_element()
            }
        }
    }

    fn render_recent_preview(
        &self,
        book: &LibraryBook,
        date: Timestamp,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = book.id.clone();
        // The preview represents the saved metadata JSON, not the source EPUB.
        let metadata = serde_json::to_vec_pretty(&book.record).unwrap_or_default();
        let name = format!("{}.json", book.title);
        let preview = FilePreview::new(
            gpui::ElementId::Name(format!("preview-{id}").into()),
            DirEntry::file(name, metadata.len() as u64, date),
        )
        .picture(
            match book
                .cover_url
                .as_deref()
                .and_then(|url| self.covers.get(url))
            {
                Some(Cover::Ready(image)) => image.clone(),
                _ => self.preview_placeholder.1.clone(),
            },
            BOOK_RATIO,
        );
        let keyboard_id = id.clone();
        div()
            .id(gpui::ElementId::Name(format!("recent-card-{id}").into()))
            .role(gpui::Role::Button)
            .aria_label(format!("Open {}", book.title))
            .aria_description(match book.gutenberg_id {
                Some(id) => format!("Project Gutenberg book {id}. Saved structural schema."),
                None => "Uploaded EPUB. Saved structural schema.".into(),
            })
            .min_w_0()
            .tab_index(0)
            .rounded(cx.theme().radius(Radius::Lg))
            .border_1()
            .border_color(cx.theme().colors.bg)
            .focus_ring(cx)
            .cursor_pointer()
            .child(preview)
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        this.select_upload(keyboard_id.clone(), window, cx);
                    }
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.select_upload(id.clone(), window, cx);
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn recents_include_seven_day_boundary_and_exclude_old_future_or_missing_dates() {
        let now: Timestamp = "2026-10-08T06:00:00Z".parse().unwrap();
        for (date, expected) in [
            ("2026-10-01T06:00:00Z", true),
            ("2026-10-01T05:59:59Z", false),
            ("2026-10-09T06:00:00Z", false),
            ("broken", false),
        ] {
            assert_eq!(
                recent_at(&json!({"created_at":date}), now).is_some(),
                expected
            );
        }
        assert!(recent_at(&json!({}), now).is_none());
    }
    #[test]
    fn recents_use_latest_recorded_activity_and_parse_timezone_offsets() {
        let now: Timestamp = "2026-10-08T06:00:00Z".parse().unwrap();
        let value = json!({"created_at":"2025-01-01T00:00:00Z","updated_at":"2026-10-08T01:00:00-04:00",
            "scan":{"last_scanned_at":"2026-10-07T00:00:00Z"}});
        assert_eq!(
            recent_at(&value, now).unwrap(),
            "2026-10-08T05:00:00Z".parse::<Timestamp>().unwrap()
        );
    }
}
