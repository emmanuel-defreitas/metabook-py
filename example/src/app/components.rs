//! Sketch library layout, using persisted records and the shared Ely palette.
use super::detail::{authors, field, language};
use super::{Cover, Library, MetabookApp, Phase};
use crate::api::LibraryBook;
use ely_gpui_component::buttons::{Button, ButtonVariant, IconButton};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::navigation::NavItem;
use ely_gpui_component::primitives::IconName as ElyIconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Radius, TextSize};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, img, rems, AnyElement, Context, ElementId, ExternalPaths, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _,
};
use gpui_component::input::Input;
use gpui_component::select::Select;
use gpui_component::skeleton::Skeleton;
use gpui_component::{h_flex, v_flex, Icon, IconName, Sizable as _, StyledExt as _};

impl MetabookApp {
    pub(super) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(rems(3.5))
            .flex_none()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().colors.border)
            .bg(cx.theme().colors.surface)
            .child(
                div().w(rems(15.)).flex_none().px_6().child(
                    div()
                        .id("library-home")
                        .cursor_pointer()
                        .font_semibold()
                        .text_size(cx.theme().text_size(TextSize::Md))
                        .child("metaBook")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.category = None;
                            this.show_dashboard(cx);
                        })),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .pl_12()
                    .pr_8()
                    .items_center()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.query)
                                .small()
                                .suffix(Icon::new(IconName::Search).small()),
                        ),
                    )
                    .child(
                        Button::new("search", "Search")
                            .primary()
                            .size(ControlSize::Sm)
                            .loading(self.is_processing())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.start_search(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("choose-epub", "Upload EPUB")
                            .variant(ButtonVariant::Outline)
                            .size(ControlSize::Sm)
                            .disabled(self.is_processing())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_dashboard(cx);
                                this.choose_epub(cx);
                            })),
                    )
                    .child(
                        IconButton::new(
                            "toggle-theme",
                            if cx.theme().is_dark() {
                                ElyIconName::Sun
                            } else {
                                ElyIconName::Moon
                            },
                        )
                        .variant(ButtonVariant::Outline)
                        .size(ControlSize::Sm)
                        .tooltip("Switch between light and dark mode")
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
                    ),
            )
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
        if let Phase::Done { title, value, .. } = &self.phase {
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
        sidebar
            .child(
                v_flex()
                    .px_6()
                    .py_10()
                    .gap_3()
                    .child(
                        div()
                            .px_2()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_muted)
                            .child("Explore"),
                    )
                    .children(
                        [
                            (None, "All books", ElyIconName::Library),
                            (Some("theology"), "Theology", ElyIconName::BookOpen),
                            (Some("philosophy"), "Philosophy", ElyIconName::Lightbulb),
                            (Some("ai"), "A.I.", ElyIconName::Bot),
                        ]
                        .into_iter()
                        .map(|(category, label, icon)| {
                            let view = cx.entity();
                            NavItem::new(
                                ElementId::Name(format!("category-{label}").into()),
                                icon,
                                label,
                            )
                            .active(self.category == category)
                            .on_click(move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.category = category;
                                    this.show_dashboard(cx);
                                    cx.notify();
                                })
                            })
                        }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_dashboard(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .id("library-page")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .pl_12()
                    .pr_8()
                    .py_9()
                    .gap_5()
                    .child(self.render_drop_zone(cx))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(cx.theme().text_size(TextSize::Sm))
                                    .text_color(cx.theme().colors.fg_muted)
                                    .child("Recents"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("scan-options", "Scan options")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ControlSize::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.scan_options = !this.scan_options;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        IconButton::new("refresh-library", ElyIconName::RefreshCw)
                                            .size(ControlSize::Sm)
                                            .tooltip("Reload the library")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.refresh_library(cx)
                                            })),
                                    ),
                            ),
                    )
                    .when(self.scan_options, |column| {
                        column.child(
                            h_flex()
                                .gap_4()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(cx.theme().text_size(TextSize::Sm))
                                        .child("Detail"),
                                )
                                .child(div().w(rems(9.)).child(Select::new(&self.detail).small()))
                                .child(
                                    div()
                                        .text_size(cx.theme().text_size(TextSize::Sm))
                                        .child("Tokenizer"),
                                )
                                .child(div().w_56().child(Select::new(&self.tokenizer).small())),
                        )
                    })
                    .child(self.render_library_rows(cx))
                    .child(
                        div()
                            .pt_5()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_muted)
                            .child("Book Structure API · Structure and counts, never book text"),
                    ),
            )
            .into_any_element()
    }

    fn render_drop_zone(&self, cx: &Context<Self>) -> impl IntoElement {
        let name = self.epub_display_name();
        let drag_bg = cx.theme().colors.info_subtle;
        let border = cx.theme().colors.focus;
        div()
            .id("epub-drop-zone")
            .w_full()
            .h_24()
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(cx.theme().colors.border)
            .bg(cx.theme().colors.surface)
            .rounded(cx.theme().radius(Radius::Lg))
            .drag_over::<ExternalPaths>(move |style, _, _, _| {
                style.bg(drag_bg).border_color(border)
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.on_epub_drop(paths, cx)))
            .child(
                v_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_color(cx.theme().colors.fg_muted)
                            .child(Icon::new(IconName::FileText).small()),
                    )
                    .child(
                        Button::new(
                            "drop-choose-epub",
                            name.clone().unwrap_or_else(|| "Upload EPUB".into()),
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .disabled(self.is_processing())
                        .on_click(cx.listener(|this, _, _, cx| this.choose_epub(cx))),
                    )
                    .when(name.is_some(), |column| {
                        column.child(
                            Button::new("analyze", "Analyze")
                                .primary()
                                .size(ControlSize::Sm)
                                .loading(self.is_processing())
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.start_upload(window, cx)
                                    }),
                                ),
                        )
                    }),
            )
    }

    fn render_library_rows(&self, cx: &Context<Self>) -> AnyElement {
        match &self.library {
            Library::Loading => v_flex()
                .gap_3()
                .children((0..3).map(|_| Skeleton::new().w_full().h(rems(7.))))
                .into_any_element(),
            Library::Failed(message) => self.render_library_notice(message.clone(), cx),
            Library::Ready(books) => {
                let books: Vec<_> = books
                    .iter()
                    .filter(|book| super::detail::matches_category(&book.record, self.category))
                    .collect();
                if books.is_empty() {
                    return self.render_library_notice(
                        "No books here yet. Search above or upload an EPUB.".into(),
                        cx,
                    );
                }
                v_flex()
                    .children(books.into_iter().map(|book| self.render_book_row(book, cx)))
                    .into_any_element()
            }
        }
    }

    fn render_library_notice(&self, message: SharedString, cx: &Context<Self>) -> AnyElement {
        div()
            .py_10()
            .text_size(cx.theme().text_size(TextSize::Sm))
            .text_color(cx.theme().colors.fg_muted)
            .child(message)
            .into_any_element()
    }

    fn render_book_row(&self, book: &LibraryBook, cx: &Context<Self>) -> impl IntoElement {
        let record = &book.record;
        let summary = &record["scan"]["summary"];
        let scanned = record["scan"]["scanned"].as_bool().unwrap_or(false);
        let confidence = record["scan"]["schema_confidence"]
            .as_str()
            .unwrap_or("Not scanned");
        let tone = match confidence {
            "high" => Tone::Success,
            "medium" | "low" => Tone::Warning,
            _ => Tone::Neutral,
        };
        let id = book.id.clone();
        let gutenberg_id = book.gutenberg_id;
        h_flex()
            .id(ElementId::Name(format!("book-{}", book.id).into()))
            .w_full()
            .gap_6()
            .py_3()
            .items_center()
            .cursor_pointer()
            .border_b_1()
            .border_color(cx.theme().colors.border)
            .hover(|style| style.bg(cx.theme().colors.hover))
            .child(
                div()
                    .w_20()
                    .h(rems(7.))
                    .flex_none()
                    .child(self.render_cover(book.cover_url.as_deref(), false, cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().truncate().child(book.title.clone()))
                    .child(
                        div()
                            .truncate()
                            .text_size(cx.theme().text_size(TextSize::Sm))
                            .text_color(cx.theme().colors.fg_muted)
                            .child(format!(
                                "{} · {} · {}",
                                authors(&record["book"]),
                                language(&record["book"]),
                                field(record, "format").to_uppercase()
                            )),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_muted)
                            .child(if scanned {
                                format!(
                                    "{} top-level nodes · {} paragraphs · {} words",
                                    field(summary, "total_top_level_nodes"),
                                    field(summary, "total_paragraphs"),
                                    field(summary, "total_words")
                                )
                            } else {
                                "Not scanned · structural summary unavailable".into()
                            }),
                    ),
            )
            .child(
                div()
                    .w(rems(7.))
                    .flex_none()
                    .child(Badge::new(super::detail::capitalized(confidence)).tone(tone)),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if scanned {
                    this.select_upload(id.clone(), window, cx)
                } else if let Some(id) = gutenberg_id {
                    this.select_match(id, window, cx)
                }
            }))
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
