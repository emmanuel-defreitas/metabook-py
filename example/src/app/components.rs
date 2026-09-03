//! Dashboard sections.
//!
//! The dashboard is the app's home: a search row over Project Gutenberg, a
//! drop zone that accepts an EPUB from the file system, and the library grid
//! of everything the API has already scanned and persisted.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, img, px, AnyElement, Context, ElementId, ExternalPaths, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::select::Select;
use gpui_component::skeleton::Skeleton;
use gpui_component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    StyledExt as _,
};

use crate::api::LibraryBook;

use super::styles::{DETAIL_FIELD_WIDTH, ISBN_FIELD_WIDTH};
use super::{Cover, Library, MetabookApp};

impl MetabookApp {
    /// The whole dashboard, and the scroll owner for its sections: the inset
    /// lives inside the scrolling element so the scrollbar stays at the panel
    /// edge rather than floating inside the padding.
    pub(super) fn render_dashboard(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .id("dashboard")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .p_4()
                    .pt_2()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_lg().font_semibold().child("Dashboard"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "Search Project Gutenberg or add an EPUB of your own. \
                                         Every book you scan is kept in the library below.",
                                    ),
                            ),
                    )
                    .child(self.render_search_row(cx))
                    .child(self.render_drop_zone(cx))
                    .child(self.render_explore(cx)),
            )
            .into_any_element()
    }

    /// Title/author and ISBN search, with the tokenizer and detail options
    /// that apply to both search and upload scans.
    fn render_search_row(&self, cx: &Context<Self>) -> impl IntoElement {
        let processing = self.is_processing();

        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(div().flex_1().min_w_0().child(Input::new(&self.query)))
            .child(div().w(px(ISBN_FIELD_WIDTH)).child(Input::new(&self.isbn)))
            .child(div().w_56().child(Select::new(&self.tokenizer)))
            .child(
                div()
                    .w(px(DETAIL_FIELD_WIDTH))
                    .child(Select::new(&self.detail)),
            )
            .child(
                Button::new("search")
                    .primary()
                    .icon(IconName::Search)
                    .label("Search")
                    .loading(processing)
                    .disabled(processing)
                    .on_click(cx.listener(|this, _, window, cx| this.start_search(window, cx))),
            )
    }

    /// EPUB intake. The zone accepts a file dragged from the operating
    /// system and highlights while one hovers over it; the button offers the
    /// same command for keyboard and pointer users who aren't dragging.
    fn render_drop_zone(&self, cx: &Context<Self>) -> impl IntoElement {
        let processing = self.is_processing();
        let epub_name = self.epub_display_name();
        let has_file = epub_name.is_some();
        // Drag styling is a `'static` closure, so resolve the tokens now.
        let drag_bg = cx.theme().accent;
        let drag_border = cx.theme().primary;

        div()
            .id("epub-drop-zone")
            .w_full()
            .h_32()
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_dashed()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .drag_over::<ExternalPaths>(move |style, _, _, _| {
                style.bg(drag_bg).border_color(drag_border)
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.on_epub_drop(paths, cx)))
            .child(
                v_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(Icon::new(IconName::FileText).large()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .truncate()
                            .child(epub_name.unwrap_or_else(|| "Drag and drop an EPUB".into())),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("choose-epub")
                                    .label("Choose EPUB…")
                                    .disabled(processing)
                                    .on_click(cx.listener(|this, _, _, cx| this.choose_epub(cx))),
                            )
                            .when(has_file, |row| {
                                row.child(
                                    Button::new("analyze")
                                        .primary()
                                        .icon(
                                            Icon::default()
                                                .path("icons/document-magnifying-glass.svg"),
                                        )
                                        .label("Analyze")
                                        .loading(processing)
                                        .disabled(processing)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.start_upload(window, cx)
                                        })),
                                )
                            }),
                    ),
            )
    }

    /// The persisted library: books stored in Vercel Blob and MongoDB by the
    /// API, newest first.
    fn render_explore(&self, cx: &Context<Self>) -> impl IntoElement {
        let total: SharedString = match &self.library {
            Library::Loading => "Loading…".into(),
            Library::Ready(books) => {
                if books.len() == 1 {
                    "1 book".into()
                } else {
                    format!("{} books", books.len()).into()
                }
            }
            Library::Failed(_) => "Unavailable".into(),
        };

        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_baseline()
                            .min_w_0()
                            .child(div().font_semibold().child("Explore"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(total),
                            ),
                    )
                    .child(
                        Button::new("refresh-library")
                            .ghost()
                            .small()
                            .icon(IconName::RotateCw)
                            .tooltip("Reload the library")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_library(cx))),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Books picked from Gutenberg searches and uploaded EPUBs, kept with their scans."),
            )
            .child(self.render_library_grid(cx))
    }

    fn render_library_grid(&self, cx: &Context<Self>) -> AnyElement {
        match &self.library {
            Library::Loading => div()
                .flex()
                .flex_wrap()
                .gap_3()
                .children((0..6).map(|_| Skeleton::new().w_40().h_72()))
                .into_any_element(),
            Library::Failed(message) => self
                .render_library_notice(message.clone(), cx)
                .into_any_element(),
            Library::Ready(books) if books.is_empty() => self
                .render_library_notice(
                    "No books yet — search above or drop an EPUB to add your first.".into(),
                    cx,
                )
                .into_any_element(),
            Library::Ready(books) => div()
                .flex()
                .flex_wrap()
                .gap_3()
                .children(books.iter().map(|book| self.render_book_card(book, cx)))
                .into_any_element(),
        }
    }

    /// Empty and failure states share one centred, muted block.
    fn render_library_notice(&self, message: SharedString, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .py_8()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(message),
            )
    }

    /// One book: its cover (Gutenberg) or a placeholder (uploaded EPUB, whose
    /// file lives in private blob storage), with the title underneath.
    ///
    /// Only Gutenberg-sourced books can be re-scanned from their id, so only
    /// those cards are interactive.
    fn render_book_card(&self, book: &LibraryBook, cx: &Context<Self>) -> impl IntoElement {
        let title = SharedString::from(book.title.clone());
        // The fallback closure outlives this render, so it rebuilds the
        // placeholder from copied tokens rather than capturing an element.
        let (muted, muted_foreground) = (cx.theme().muted, cx.theme().muted_foreground);
        let placeholder = move || {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .bg(muted)
                .text_color(muted_foreground)
                .child(Icon::new(IconName::BookOpen).large())
                .into_any_element()
        };

        // Covers are fetched by the app (see `load_covers`) because GPUI's own
        // URL loader has no HTTP client on native.
        let cover_state = book
            .cover_url
            .as_ref()
            .and_then(|url| self.covers.get(url.as_str()));
        let cover = div()
            .w_full()
            .h_56()
            .overflow_hidden()
            .child(match cover_state {
                Some(Cover::Ready(image)) => img(image.clone())
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_fallback(placeholder)
                    .into_any_element(),
                Some(Cover::Loading) => Skeleton::new().size_full().into_any_element(),
                // Failed, or a book with no cover at all (an uploaded EPUB).
                _ => placeholder(),
            });

        let card = v_flex()
            .id(ElementId::Name(format!("book-{}", book.id).into()))
            .w_40()
            .flex_none()
            .overflow_hidden()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .child(cover)
            .child(
                div()
                    .w_full()
                    .px_2()
                    .py_2()
                    .text_sm()
                    .truncate()
                    .child(title),
            );

        match book.gutenberg_id {
            Some(gutenberg_id) => {
                card.hover(|style| style.bg(cx.theme().muted))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_match(gutenberg_id, window, cx)
                    }))
            }
            None => card,
        }
    }
}
