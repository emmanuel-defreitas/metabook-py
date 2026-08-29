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
    AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::list::ListItem;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::text::TextView;
use gpui_component::tree::{TreeItem, TreeState, tree};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Root, Sizable as _, StyledExt as _,
    h_flex, v_flex,
};

use crate::api::{self, BookMatch, SearchOutcome, TreeNode};

const TAB_SEARCH: usize = 0;
const TAB_UPLOAD: usize = 1;

/// The workflow phase shown in the content region.
enum Phase {
    Idle,
    Processing { message: SharedString },
    /// A search matched several books; the user picks one to analyse.
    Matches { matches: Vec<BookMatch> },
    Done {
        title: SharedString,
        schema_json: SharedString,
        tree_state: Entity<TreeState>,
    },
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
            "Searching Gutendex…",
            move || api::search(&base, &query, &isbn),
            cx,
        );
    }

    fn select_match(&mut self, gutenberg_id: u64, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let base = self.api_base.to_string();
        self.begin_request(
            "Fetching and scanning the book text…",
            move || api::fetch_by_id(&base, gutenberg_id).map(SearchOutcome::Analysis),
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
            move || api::upload(&base, &path).map(SearchOutcome::Analysis),
            cx,
        );
    }

    fn begin_request(
        &mut self,
        message: &'static str,
        work: impl FnOnce() -> Result<SearchOutcome, String> + Send + 'static,
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

    fn finish_request(
        &mut self,
        ix: usize,
        result: Result<SearchOutcome, String>,
        cx: &mut Context<Self>,
    ) {
        if ix != self.request_ix {
            return; // A newer request superseded this one.
        }
        self.phase = match result {
            Ok(SearchOutcome::Analysis(analysis)) => {
                let items: Vec<TreeItem> =
                    analysis.tree.iter().map(|n| to_tree_item(n, true)).collect();
                Phase::Done {
                    title: analysis.title.into(),
                    schema_json: analysis.schema_json.into(),
                    tree_state: cx.new(|cx| TreeState::new(cx).items(items)),
                }
            }
            Ok(SearchOutcome::Matches(matches)) => Phase::Matches { matches },
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
            Phase::Matches { matches } => self.render_matches(matches.clone(), cx).into_any_element(),
            Phase::Failed { message } => self.render_failed(message.clone(), cx).into_any_element(),
            Phase::Done { title, schema_json, tree_state } => self
                .render_result(title.clone(), schema_json.clone(), tree_state.clone(), cx)
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

    fn render_matches(&self, matches: Vec<BookMatch>, cx: &Context<Self>) -> impl IntoElement {
        let count = matches.len();
        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{count} books matched — select one to analyse")),
            )
            .child(
                v_flex()
                    .id("match-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(matches.into_iter().map(|book| {
                        let id = book.gutenberg_id;
                        let subtitle = if book.language.is_empty() {
                            format!("#{id}")
                        } else {
                            format!("#{id} · {}", book.language)
                        };
                        h_flex()
                            .id(("match", id))
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .px_3()
                            .py_2()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius)
                            .hover(|style| style.bg(cx.theme().muted))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .min_w_0()
                                    .child(div().truncate().child(book.title))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(book.authors),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .flex_none()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(subtitle),
                                    )
                                    .child(
                                        Button::new(("analyse-match", id))
                                            .small()
                                            .label("Schema")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_match(id, cx)
                                            })),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| this.select_match(id, cx)))
                    })),
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
        tree_state: Entity<TreeState>,
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
                div().flex_1().min_h_0().child(
                    h_resizable("result-split")
                        .child(
                            resizable_panel()
                                .size(px(300.))
                                .size_range(px(200.)..px(560.))
                                .child(self.render_structure_tree(tree_state, cx)),
                        )
                        .child(
                            resizable_panel().child(
                                div().size_full().pl_3().child(
                                    TextView::markdown("schema-json", markdown)
                                        .selectable(true)
                                        .scrollable(true),
                                ),
                            ),
                        ),
                ),
            )
    }

    fn render_structure_tree(
        &self,
        tree_state: Entity<TreeState>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .pr_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(tree(&tree_state, |ix, entry, selected, _, _| {
                let icon = if !entry.is_folder() {
                    IconName::File
                } else if entry.is_expanded() {
                    IconName::FolderOpen
                } else {
                    IconName::Folder
                };
                ListItem::new(ix)
                    .selected(selected)
                    .pl(px(16.) * entry.depth() as f32 + px(4.))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(icon).small())
                            .child(div().text_sm().truncate().child(entry.item().label.clone())),
                    )
            }))
    }
}

/// Convert an API tree node into a `TreeItem`; only top-level nodes start expanded.
fn to_tree_item(node: &TreeNode, expanded: bool) -> TreeItem {
    TreeItem::new(
        SharedString::from(node.id.clone()),
        SharedString::from(node.label.clone()),
    )
    .expanded(expanded)
    .children(node.children.iter().map(|c| to_tree_item(c, false)).collect::<Vec<_>>())
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
