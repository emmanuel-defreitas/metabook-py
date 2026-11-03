//! Catalog submission, immutable pagination snapshots, and provider-specific actions.

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    theme::{ActiveTheme as _, ControlSize, Radius, TextSize},
};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, AnyElement, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window,
};
use gpui_component::{h_flex, v_flex};

use super::{
    components::{render_score, schema_score},
    Library, MetabookApp, Phase,
};
use crate::api::{self, BookMatch, CatalogFilters, CatalogSource, SearchPage};

impl MetabookApp {
    pub(super) fn start_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let query = self.query.read(cx).text().trim().to_string();
        if query.is_empty() {
            self.workspace.next = Some("search".into());
            self.open_request_tab(cx);
            self.workspace.pending = None;
            self.workspace.title = "Search results".into();
            self.phase = Phase::Failed {
                message: "Enter catalog keywords, an ISBN, or a Gutenberg ID to search.".into(),
            };
            cx.notify();
            return;
        }
        let filters = CatalogFilters {
            date: self.filters.date.clone(),
            category: self.filters.category.clone(),
            kind: self.filters.kind.clone(),
            source: vec![self.filters.source.clone()],
        };
        self.search_page(query, 1, filters, window, cx);
    }

    fn search_page(
        &mut self,
        query: String,
        page: u64,
        filters: CatalogFilters,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_processing() {
            return;
        }
        self.workspace.next = Some("search".into());
        let base = self.api_base.to_string();
        self.begin_request(
            "Searching catalog metadata…",
            false,
            true,
            move || api::search(&base, &query, page, &filters),
            window,
            cx,
        );
    }

    pub(super) fn render_matches(&self, page: SearchPage, cx: &Context<Self>) -> impl IntoElement {
        let previous_query = page.query.clone();
        let next_query = page.query.clone();
        let previous_filters = page.filters.clone();
        let next_filters = page.filters.clone();
        let sources = if page.filters.source.is_empty() {
            "Gutendex".to_string()
        } else {
            page.filters.source.join(" + ")
        };
        let heading = if page.count == 0 {
            format!("No catalog results in {sources}. Try another query or change the filters.")
        } else {
            format!(
                "{} catalog results · {sources} · Page {}",
                page.count, page.page
            )
        };
        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(heading),
            )
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .text_color(cx.theme().colors.fg_subtle)
                    .child("Gutenberg: Schema analyses the book. OpenAlex: Open record visits the catalog; it does not scan or save a schema. Filter edits apply on the next Enter submission."),
            )
            .children(page.warnings.into_iter().map(|warning| {
                div().text_color(cx.theme().colors.fg_muted).child(warning)
            }))
            .child(
                v_flex()
                    .id("match-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(page.matches.into_iter().map(|book| self.render_catalog_match(book, cx))),
            )
            .child(
                h_flex()
                    .gap_2()
                    .when_some(page.previous_page, |row, previous| {
                        row.child(
                            Button::new("search-previous", "Previous")
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Outline)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.search_page(previous_query.clone(), previous, previous_filters.clone(), window, cx);
                                })),
                        )
                    })
                    .when_some(page.next_page, |row, next| {
                        row.child(
                            Button::new("search-next", "Next")
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Outline)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.search_page(next_query.clone(), next, next_filters.clone(), window, cx);
                                })),
                        )
                    }),
            )
    }

    fn render_catalog_match(&self, book: BookMatch, cx: &Context<Self>) -> AnyElement {
        let key = book.source.key();
        let subtitle = catalog_subtitle(&book);
        let row_source = book.source.clone();
        let (score, action) = match &book.source {
            CatalogSource::Gutendex(id) => {
                let id = *id;
                let score = match &self.library {
                    Library::Ready(books) => books
                        .iter()
                        .find(|saved| saved.gutenberg_id == Some(id))
                        .and_then(|saved| schema_score(&saved.record)),
                    _ => None,
                };
                let action =
                    Button::new(ElementId::Name(format!("analyse-{key}").into()), "Schema")
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.select_match(id, window, cx);
                        }))
                        .into_any_element();
                (score, action)
            }
            CatalogSource::OpenAlex(_) => {
                let url = book.source.openalex_url();
                let action =
                    Button::new(ElementId::Name(format!("open-{key}").into()), "Open record")
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Outline)
                        .disabled(url.is_none())
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(url) = &url {
                                cx.open_url(url);
                            }
                        })
                        .into_any_element();
                (None, action)
            }
        };
        h_flex()
            .id(ElementId::Name(format!("match-{key}").into()))
            .hover(|style| style.bg(cx.theme().colors.hover.opacity(0.5)))
            .cursor_pointer()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .border_1()
            .bg(cx.theme().colors.surface)
            .border_color(cx.theme().colors.border)
            .rounded(cx.theme().radius(Radius::Lg))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().truncate().child(book.title))
                    .child(
                        div()
                            .text_size(cx.theme().text_size(TextSize::Sm))
                            .text_color(cx.theme().colors.fg_muted)
                            .truncate()
                            .child(book.authors),
                    )
                    .child(
                        div()
                            .text_size(cx.theme().text_size(TextSize::Xs))
                            .text_color(cx.theme().colors.fg_subtle)
                            .child(subtitle),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .flex_none()
                    .child(render_score(score, cx))
                    .child(action),
            )
            .on_click(cx.listener(move |this, _, window, cx| match &row_source {
                CatalogSource::Gutendex(id) => this.select_match(*id, window, cx),
                CatalogSource::OpenAlex(_) => {
                    if let Some(url) = row_source.openalex_url() {
                        cx.open_url(&url);
                    }
                }
            }))
            .into_any_element()
    }
}

fn catalog_subtitle(book: &BookMatch) -> String {
    let source = match &book.source {
        CatalogSource::Gutendex(id) => format!("Gutendex · #{id}"),
        CatalogSource::OpenAlex(id) => format!("OpenAlex · {id}"),
    };
    [
        Some(source.as_str()),
        (!book.kind.is_empty()).then_some(book.kind.as_str()),
        (!book.language.is_empty()).then_some(book.language.as_str()),
        book.publication_date.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}
