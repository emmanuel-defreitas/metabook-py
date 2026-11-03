//! Book-aware file previews with a stable portrait cover frame.

use ely_gpui_component::{
    lists::DirEntry,
    primitives::{Icon, Image},
    theme::{ActiveTheme as _, IconSize, Radius, TextSize},
    typography::{format, Ellipsis, MiddleEllipsis},
};
use gpui::{
    div, relative, App, ElementId, FontWeight, ImageSource, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, Styled, Window,
};
use jiff::Timestamp;

use super::score::render_score;

const COVER_RATIO: f32 = 2.0 / 3.0;

/// A saved book's JSON file, with a cover or a closed-book placeholder.
#[derive(IntoElement)]
pub(in crate::app) struct MFilePreview {
    id: ElementId,
    entry: DirEntry,
    cover: Option<ImageSource>,
    score: Option<f64>,
    modified: Option<Timestamp>,
}

impl MFilePreview {
    pub(in crate::app) fn new(id: impl Into<ElementId>, entry: DirEntry) -> Self {
        let modified = Some(entry.modified());
        Self {
            id: id.into(),
            entry,
            cover: None,
            score: None,
            modified,
        }
    }

    pub(in crate::app) fn picture(mut self, source: impl Into<ImageSource>) -> Self {
        self.cover = Some(source.into());
        self
    }

    pub(in crate::app) fn score(mut self, score: Option<f64>) -> Self {
        self.score = score;
        self
    }

    pub(in crate::app) fn modified(mut self, modified: Option<Timestamp>) -> Self {
        self.modified = modified;
        self
    }
}

impl RenderOnce for MFilePreview {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let score = div()
            .id((self.id.clone(), "score"))
            .child(render_score(self.score, cx));
        let theme = cx.theme();
        let round = theme.radius(Radius::Lg);
        let mut stage = div()
            .debug_selector(|| "file-preview-stage".into())
            .relative()
            .w(relative(0.5))
            .overflow_hidden()
            .rounded(round)
            .bg(theme.colors.sunken);
        // Both states use the same geometry so a loaded cover never shifts the captions.
        stage.style().aspect_ratio = Some(COVER_RATIO);
        let stage = match self.cover {
            Some(source) => stage.child(
                Image::new((self.id, "picture"), source)
                    .size_full()
                    .rounded(round),
            ),
            None => stage.flex().items_center().justify_center().child(
                Icon::from_path(crate::BOOK_ICON)
                    .size(IconSize::Xxl)
                    .color(theme.colors.fg_muted),
            ),
        };
        let detail = match self.entry.size() {
            Some(bytes) => format!("JSON · {}", format::file_size(bytes, false)),
            None => "JSON".into(),
        };
        let muted = |text: String| {
            div()
                .text_size(theme.text_size(TextSize::Sm))
                .text_color(theme.colors.fg_muted)
                .child(Ellipsis::new(text))
        };
        div()
            .debug_selector(|| "file-preview".into())
            .flex()
            .flex_col()
            .min_w_0()
            .gap_3()
            .child(stage)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(theme.text_size(TextSize::Md))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.colors.fg)
                            .child(MiddleEllipsis::new(self.entry.name().clone())),
                    )
                    .child(muted(detail))
                    .child(muted(match self.modified {
                        Some(date) => {
                            format!("Changed {}", format::relative(date, Timestamp::now()))
                        }
                        None => "Saved date unavailable".into(),
                    }))
                    .child(score),
            )
    }
}
