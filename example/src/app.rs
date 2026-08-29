//! Main application view.
//!
//! One utility-window workflow: pick a source (search Gutenberg or upload an
//! EPUB), watch a processing state while the API fetches and scans the book,
//! then read the returned structural schema as code.
//!
//! State ownership: `MetabookApp` owns the workflow phase and the two input
//! states. Async requests carry a request index so a stale response can never
//! overwrite a newer one.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, ClipboardItem, Context, Entity, IntoElement, ParentElement,
    PathPromptOptions, Render, SharedString, Styled, Subscription, Window, div,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::text::TextView;
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Root, Sizable as _, StyledExt as _, h_flex,
    v_flex,
};

use crate::api::{self, Analysis};

const TAB_SEARCH: usize = 0;
const TAB_UPLOAD: usize = 1;

/// The workflow phase shown in the content region.
enum Phase {
    Idle,
    Processing { message: SharedString },
    Done { title: SharedString, schema_json: SharedString },
    Failed { message: SharedString },
}

pub struct MetabookApp {
    api_base: SharedString,
    tab_ix: usize,
    query: Entity<InputState>,
    isbn: Entity<InputState>,
    epub_path: Option<PathBuf>,
    phase: Phase,
    /// Incremented per request; responses for an older index are discarded.
    request_ix: usize,
    _subscriptions: Vec<Subscription>,
}

impl MetabookApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 127.0.0.1 rather than localhost: on hosts where another service
        // (e.g. a container runtime) listens on *:8000, localhost can resolve
        // to ::1 and reach that service instead of the local API.
        let api_base = std::env::var("METABOOK_API")
            .unwrap_or_else(|_| "http://127.0.0.1:8000".into())
            .trim_end_matches('/')
            .to_string();

        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Title or author, e.g. Pride and Prejudice")
        });
        let isbn = cx.new(|cx| InputState::new(window, cx).placeholder("ISBN-10 or ISBN-13"));

        let subscriptions = vec![
            cx.subscribe_in(&query, window, Self::on_input_event),
            cx.subscribe_in(&isbn, window, Self::on_input_event),
        ];

        Self {
            api_base: api_base.into(),
            tab_ix: TAB_SEARCH,
            query,
            isbn,
            epub_path: None,
            phase: Phase::Idle,
            request_ix: 0,
            _subscriptions: subscriptions,
        }
    }

    fn on_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::PressEnter { .. } = event {
            self.start_search(cx);
        }
    }

    fn is_processing(&self) -> bool {
        matches!(self.phase, Phase::Processing { .. })
    }

    // ── Commands ───────────────────────────────────────────────────────────────

    fn start_search(&mut self, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let query = self.query.read(cx).value().trim().to_string();
        let isbn = self.isbn.read(cx).value().trim().to_string();
        if query.is_empty() && isbn.is_empty() {
            self.phase = Phase::Failed {
                message: "Enter a title, an author, or an ISBN to search.".into(),
            };
            cx.notify();
            return;
        }

        let base = self.api_base.to_string();
        self.begin_request(
            "Searching Gutendex and fetching the book text…",
            move || api::search(&base, &query, &isbn),
            cx,
        );
    }

    fn start_upload(&mut self, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let Some(path) = self.epub_path.clone() else {
            return;
        };

        let base = self.api_base.to_string();
        self.begin_request(
            "Uploading and scanning the EPUB…",
            move || api::upload(&base, &path),
            cx,
        );
    }

    fn begin_request(
        &mut self,
        message: &'static str,
        work: impl FnOnce() -> Result<Analysis, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.request_ix += 1;
        let ix = self.request_ix;
        self.phase = Phase::Processing { message: message.into() };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { work() }).await;
            this.update(cx, |this, cx| this.finish_request(ix, result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_request(&mut self, ix: usize, result: Result<Analysis, String>, cx: &mut Context<Self>) {
        if ix != self.request_ix {
            return; // A newer request superseded this one.
        }
        self.phase = match result {
            Ok(analysis) => Phase::Done {
                title: analysis.title.into(),
                schema_json: analysis.schema_json.into(),
            },
            Err(message) => Phase::Failed { message: message.into() },
        };
        cx.notify();
    }

    fn choose_epub(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(path) = paths.into_iter().next() {
                    this.update(cx, |this, cx| this.set_epub_path(path, cx)).ok();
                }
            }
        })
        .detach();
    }

    fn set_epub_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("epub")) {
            self.epub_path = Some(path);
            if matches!(self.phase, Phase::Failed { .. }) {
                self.phase = Phase::Idle;
            }
        } else {
            self.phase = Phase::Failed {
                message: format!("“{}” isn't an .epub file.", path.display()).into(),
            };
        }
        cx.notify();
    }

    fn copy_schema(&mut self, cx: &mut Context<Self>) {
        if let Phase::Done { schema_json, .. } = &self.phase {
            cx.write_to_clipboard(ClipboardItem::new_string(schema_json.to_string()));
        }
    }

    // ── Regions ────────────────────────────────────────────────────────────────

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_lg().font_semibold().child("Metabook"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Structural schema for any book — no text ever leaves the API"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.api_base.clone()),
            )
    }

    fn render_search_form(&self, cx: &Context<Self>) -> impl IntoElement {
        let processing = self.is_processing();
        h_flex()
            .gap_2()
            .items_center()
            .child(div().flex_1().child(Input::new(&self.query)))
            .child(div().w_48().child(Input::new(&self.isbn)))
            .child(
                Button::new("search")
                    .primary()
                    .icon(IconName::Search)
                    .label("Search")
                    .loading(processing)
                    .disabled(processing)
                    .on_click(cx.listener(|this, _, _, cx| this.start_search(cx))),
            )
    }

    fn render_upload_form(&self, cx: &Context<Self>) -> impl IntoElement {
        let processing = self.is_processing();
        let chosen: SharedString = match &self.epub_path {
            Some(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
                .into(),
            None => "No file chosen".into(),
        };

        h_flex()
            .gap_2()
            .items_center()
            .child(
                Button::new("choose-epub")
                    .label("Choose EPUB…")
                    .disabled(processing)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_epub(cx))),
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
            .child(
                Button::new("analyze")
                    .primary()
                    .icon(IconName::ArrowUp)
                    .label("Analyze")
                    .loading(processing)
                    .disabled(processing || self.epub_path.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.start_upload(cx))),
            )
    }

    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let content = match &self.phase {
            Phase::Idle => self.render_idle(cx).into_any_element(),
            Phase::Processing { message } => self.render_processing(message.clone(), cx).into_any_element(),
            Phase::Failed { message } => self.render_failed(message.clone(), cx).into_any_element(),
            Phase::Done { title, schema_json } => self
                .render_result(title.clone(), schema_json.clone(), cx)
                .into_any_element(),
        };
        div().flex_1().min_h_0().child(content)
    }

    fn render_idle(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .text_sm()
                    .child("Search Project Gutenberg or upload an EPUB."),
            )
            .child(
                div()
                    .text_sm()
                    .child("The detected structural schema appears here as code."),
            )
    }

    fn render_processing(&self, message: SharedString, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(Spinner::new().large())
            .child(div().child(message))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Fetching, cleaning, and scanning the document can take a few seconds."),
            )
    }

    fn render_failed(&self, message: SharedString, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .gap_2()
                    .max_w_96()
                    .p_4()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_color(cx.theme().danger)
                            .child(gpui_component::Icon::new(IconName::CircleX).small())
                            .child(div().font_semibold().child("Request failed")),
                    )
                    .child(div().text_sm().child(message)),
            )
    }

    fn render_result(
        &self,
        title: SharedString,
        schema_json: SharedString,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let markdown: SharedString = format!("```json\n{schema_json}\n```").into();
        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_semibold().child(title))
                    .child(
                        Button::new("copy-schema")
                            .ghost()
                            .small()
                            .icon(IconName::Copy)
                            .label("Copy JSON")
                            .on_click(cx.listener(|this, _, _, cx| this.copy_schema(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(TextView::markdown("schema-json", markdown).selectable(true).scrollable(true)),
            )
    }
}

impl Render for MetabookApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .gap_3()
            .p_4()
            .child(self.render_header(cx))
            .child(
                TabBar::new("source-tabs")
                    .selected_index(self.tab_ix)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.tab_ix = *ix;
                        cx.notify();
                    }))
                    .child(Tab::new().label("Search"))
                    .child(Tab::new().label("Upload EPUB")),
            )
            .map(|this| {
                if self.tab_ix == TAB_UPLOAD {
                    this.child(self.render_upload_form(cx))
                } else {
                    this.child(self.render_search_form(cx))
                }
            })
            .child(self.render_content(cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
