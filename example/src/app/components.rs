//! Sketch library layout, using persisted records and the shared Ely palette.

mod file_preview;
mod score;
mod search_filters;
mod theme_toggle;

pub(super) use file_preview::MFilePreview;
pub(super) use score::{render_score, schema_score};

use super::detail::{authors, field, language};
use super::{Cover, MetabookApp, Phase};
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::primitives::IconName as ElyIconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Radius, TextSize};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, img, rems, AnyElement, Context, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, StyledImage as _,
};
use gpui_component::skeleton::Skeleton;
use gpui_component::{v_flex, Icon, IconName, Sizable as _, StyledExt as _};

impl MetabookApp {
    pub(super) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        self.render_workspace_toolbar(cx)
    }

    pub(super) fn render_sidebar(&self, cx: &Context<Self>) -> AnyElement {
        let sidebar = v_flex()
            .id("app-sidebar")
            .w(rems(15.))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .bg(cx.theme().colors.sunken)
            .border_r_1()
            .border_color(cx.theme().colors.border);
        if matches!(
            self.phase,
            Phase::Processing {
                detail_loading: true,
                ..
            }
        ) {
            return sidebar
                .child(
                    v_flex()
                        .px_8()
                        .py_10()
                        .gap_4()
                        .child(
                            ely_gpui_component::motion::Skeleton::new("loading-cover")
                                .w_full()
                                .h_56(),
                        )
                        .child(ely_gpui_component::motion::SkeletonText::new(
                            "loading-book-meta",
                            3,
                        )),
                )
                .into_any_element();
        }
        if let Phase::Done { explorer, .. } = &self.phase {
            let result = explorer.read(cx);
            let title = result.title();
            let value = result.value();
            let record = self.result_record(value);
            let cover = record.and_then(|record| record.cover_url.as_deref());
            let book = &value["book"];
            return sidebar
                .child(
                    v_flex()
                        .px_8()
                        .py_10()
                        .gap_4()
                        .child(
                            div()
                                .w(rems(11.))
                                .h_56()
                                .child(self.render_cover(cover, true, cx)),
                        )
                        .child(
                            div()
                                .text_size(cx.theme().text_size(TextSize::Lg))
                                .font_semibold()
                                .child(title.clone()),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_color(cx.theme().colors.fg_muted)
                                        .child(authors(book)),
                                )
                                .child(
                                    div()
                                        .text_size(cx.theme().text_size(TextSize::Xs))
                                        .text_color(cx.theme().colors.fg_muted)
                                        .child(format!(
                                            "{} · {}",
                                            if book["source"] == "upload" {
                                                "Uploaded EPUB"
                                            } else {
                                                "Project Gutenberg"
                                            },
                                            language(book)
                                        )),
                                ),
                        )
                        .child(div().child(Badge::new("Scanned").tone(Tone::Success)))
                        .child(
                            Button::new("back-library", "Library")
                                .variant(ButtonVariant::Ghost)
                                .size(ControlSize::Sm)
                                .icon(ElyIconName::ArrowLeft)
                                .on_click(cx.listener(|this, _, _, cx| this.show_dashboard(cx))),
                        )
                        .child(
                            div()
                                .pt_3()
                                .text_size(cx.theme().text_size(TextSize::Xs))
                                .text_color(cx.theme().colors.fg_muted)
                                .child(format!("Record ID\n{}", field(value, "record_id"))),
                        )
                        .when(value["blob"]["url"].is_string(), |column| {
                            let url = value["blob"]["url"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            column.child(
                                Button::new("source-file", "Source file")
                                    .variant(ButtonVariant::Link)
                                    .size(ControlSize::Sm)
                                    .on_click(move |_, _, cx| cx.open_url(&url)),
                            )
                        }),
                )
                .into_any_element();
        }
        sidebar.into_any_element()
    }

    fn render_cover(&self, url: Option<&str>, caption: bool, cx: &Context<Self>) -> AnyElement {
        let placeholder = v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .text_color(cx.theme().colors.fg_muted)
            .child(Icon::new(IconName::BookOpen).large());
        let cover = div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(cx.theme().colors.surface)
            .border_1()
            .border_color(cx.theme().colors.border)
            .rounded(cx.theme().radius(Radius::Md));
        match url.and_then(|url| self.covers.get(url)) {
            Some(Cover::Ready(image)) => cover
                .child(
                    img(image.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                )
                .into_any_element(),
            Some(Cover::Loading) => cover.child(Skeleton::new().size_full()).into_any_element(),
            _ => cover
                .child(placeholder)
                .when(caption, |cover| {
                    cover.child(
                        div()
                            .absolute()
                            .bottom_1()
                            .left_1()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_muted)
                            .child("Cover not available"),
                    )
                })
                .into_any_element(),
        }
    }
}
