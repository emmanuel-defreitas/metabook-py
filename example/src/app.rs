//! Main application view.
//!
//! One utility-window workflow: pick a source (search Gutenberg or upload an
//! EPUB), watch a processing state while the API fetches and scans the book,
//! then read the returned structural schema as code.
//!
//! State ownership: `MetabookApp` owns the workflow phase and the two input
//! states. Async requests carry a request index so a stale response can never
//! overwrite a newer one.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Animation, AnimationExt as _, AppContext as _, ClipboardItem, Context, ElementId, Entity,
    InteractiveElement as _, IntoElement, ParentElement, PathPromptOptions, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement as _, Styled, Subscription,
    UniformListScrollHandle, Window, div, px, radians, uniform_list,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::list::ListItem;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::select::{Select, SelectState};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::tree::{TreeItem, TreeState, tree};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Root, Sizable as _, StyledExt as _,
    Theme, ThemeMode, TitleBar, h_flex, v_flex,
};

use crate::api::{self, BookMatch, SearchOutcome, TreeNode};

const TAB_SEARCH: usize = 0;
const TAB_UPLOAD: usize = 1;

/// Options for the detail Select, index-aligned with `DETAIL_VALUES`.
const DETAIL_OPTIONS: [&str; 4] = ["Paragraphs", "Sentences", "Clauses", "Words"];
const DETAIL_VALUES: [&str; 4] = ["paragraph", "sentence", "clause", "word"];

/// The workflow phase shown in the content region.
enum Phase {
    Idle,
    Processing { message: SharedString },
    /// A search matched several books; the user picks one to analyse.
    Matches { matches: Vec<BookMatch> },
    Done {
        title: SharedString,
        schema_json: SharedString,
        json_lines: Vec<SharedString>,
        /// Node id → (first, last) 0-based JSON line of that node.
        ranges: HashMap<String, (usize, usize)>,
        /// Lines currently highlighted from the tree selection.
        highlight: Option<(usize, usize)>,
        tree_state: Entity<TreeState>,
        json_scroll: UniformListScrollHandle,
    },
    Failed { message: SharedString },
}

pub struct MetabookApp {
    api_base: SharedString,
    tab_ix: usize,
    query: Entity<InputState>,
    isbn: Entity<InputState>,
    detail: Entity<SelectState<Vec<&'static str>>>,
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
        // Default to sentence detail so the deeper nesting is visible without
        // requesting word-level nodes for every large book up front.
        let detail = cx.new(|cx| {
            SelectState::new(
                DETAIL_OPTIONS.to_vec(),
                Some(gpui_component::IndexPath::new(1)),
                window,
                cx,
            )
        });

        let subscriptions = vec![
            cx.subscribe_in(&query, window, Self::on_input_event),
            cx.subscribe_in(&isbn, window, Self::on_input_event),
        ];

        Self {
            api_base: api_base.into(),
            tab_ix: TAB_SEARCH,
            query,
            isbn,
            detail,
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

    /// The API `detail` value for the current Select choice.
    fn detail_value(&self, cx: &Context<Self>) -> String {
        self.detail
            .read(cx)
            .selected_value()
            .and_then(|label| {
                DETAIL_OPTIONS
                    .iter()
                    .position(|o| o == label)
                    .map(|ix| DETAIL_VALUES[ix])
            })
            .unwrap_or("paragraph")
            .to_string()
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
        let detail = self.detail_value(cx);
        self.begin_request(
            "Searching Gutendex…",
            move || api::search(&base, &query, &isbn, &detail),
            cx,
        );
    }

    fn select_match(&mut self, gutenberg_id: u64, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let base = self.api_base.to_string();
        let detail = self.detail_value(cx);
        self.begin_request(
            "Fetching and scanning the book text…",
            move || api::fetch_by_id(&base, gutenberg_id, &detail).map(SearchOutcome::Analysis),
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
        let detail = self.detail_value(cx);
        self.begin_request(
            "Uploading and scanning the EPUB…",
            move || api::upload(&base, &path, &detail).map(SearchOutcome::Analysis),
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
                let tree_state = cx.new(|cx| TreeState::new(cx).items(items));
                // No selection event exists; observe the state and react to
                // whatever entry is selected after each change.
                cx.observe(&tree_state, Self::on_tree_changed).detach();
                Phase::Done {
                    title: analysis.title.into(),
                    json_lines: analysis
                        .schema_json
                        .lines()
                        .map(|l| SharedString::from(l.to_string()))
                        .collect(),
                    schema_json: analysis.schema_json.into(),
                    ranges: analysis.ranges,
                    highlight: None,
                    tree_state,
                    json_scroll: UniformListScrollHandle::new(),
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

    /// After any tree change, sync the JSON pane to the selected node.
    fn on_tree_changed(&mut self, state: Entity<TreeState>, cx: &mut Context<Self>) {
        let selected_id = state
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.to_string());
        let Phase::Done { ranges, highlight, json_scroll, .. } = &mut self.phase else {
            return;
        };
        let range = selected_id.and_then(|id| ranges.get(&id).copied());
        if *highlight != range {
            *highlight = range;
            if let Some((start, _)) = range {
                json_scroll.scroll_to_item(start, ScrollStrategy::Top);
            }
            cx.notify();
        }
    }

    fn copy_schema(&mut self, cx: &mut Context<Self>) {
        if let Phase::Done { schema_json, .. } = &self.phase {
            cx.write_to_clipboard(ClipboardItem::new_string(schema_json.to_string()));
        }
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = if cx.theme().is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        Theme::change(mode, Some(window), cx);
        cx.notify();
    }

    // ── Regions ────────────────────────────────────────────────────────────────

    /// Custom title bar: app identity on the left, API base and the theme
    /// toggle on the right. Replaces the native macOS title bar.
    fn render_title_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme_icon = if cx.theme().is_dark() {
            IconName::Sun
        } else {
            IconName::Moon
        };
        TitleBar::new().child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .pr_2()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .min_w_0()
                        .child(div().text_sm().font_semibold().child("Metabook"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child("Structural schema for any book"),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_none()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.api_base.clone()),
                        )
                        .child(
                            Button::new("toggle-theme")
                                .ghost()
                                .small()
                                .icon(theme_icon)
                                .tooltip("Switch between light and dark mode")
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.toggle_theme(window, cx)
                                    }),
                                ),
                        ),
                ),
        )
    }

    fn render_search_form(&self, cx: &Context<Self>) -> impl IntoElement {
        let processing = self.is_processing();
        h_flex()
            .gap_2()
            .items_center()
            .child(div().flex_1().child(Input::new(&self.query)))
            .child(div().w_48().child(Input::new(&self.isbn)))
            .child(div().w_40().child(Select::new(&self.detail)))
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
            .child(div().w_40().child(Select::new(&self.detail)))
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
            Phase::Done { title, json_lines, tree_state, json_scroll, .. } => self
                .render_result(
                    title.clone(),
                    json_lines.len(),
                    tree_state.clone(),
                    json_scroll.clone(),
                    cx,
                )
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
        line_count: usize,
        tree_state: Entity<TreeState>,
        json_scroll: UniformListScrollHandle,
        cx: &Context<Self>,
    ) -> impl IntoElement {
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
                                div()
                                    .size_full()
                                    .pl_3()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_sm()
                                    .child(
                                        uniform_list(
                                            "schema-json-lines",
                                            line_count,
                                            cx.processor(Self::render_json_lines),
                                        )
                                        .size_full()
                                        .track_scroll(&json_scroll),
                                    ),
                            ),
                        ),
                ),
            )
    }

    fn render_json_lines(
        &mut self,
        visible: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let Phase::Done { json_lines, highlight, .. } = &self.phase else {
            return Vec::new();
        };
        let muted = cx.theme().muted;
        visible
            .filter_map(|ix| {
                let text = json_lines.get(ix)?.clone();
                let highlighted =
                    highlight.is_some_and(|(start, end)| ix >= start && ix <= end);
                Some(
                    div()
                        .px_2()
                        .whitespace_nowrap()
                        .when(highlighted, |line| line.bg(muted))
                        .child(text)
                        .into_any_element(),
                )
            })
            .collect()
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
                let id = entry.item().id.clone();
                let expanded = entry.is_expanded();

                // Folders get a chevron that rotates 0° → 90° on expand (and
                // back); keying the animation on the expanded state replays
                // it on every toggle.
                let icon = if entry.is_folder() {
                    Icon::new(IconName::ChevronRight)
                        .small()
                        .with_animation(
                            ElementId::Name(format!("chev-{id}-{expanded}").into()),
                            Animation::new(Duration::from_millis(150)),
                            move |chevron, delta| {
                                let progress = if expanded { delta } else { 1.0 - delta };
                                chevron.rotate(radians(progress * FRAC_PI_2))
                            },
                        )
                        .into_any_element()
                } else {
                    Icon::new(IconName::File).small().into_any_element()
                };

                ListItem::new(ix)
                    .selected(selected)
                    .pl(px(16.) * entry.depth() as f32 + px(4.))
                    .child(
                        // Rows fade in as they appear (newly revealed children
                        // after an expand, or rows entering the viewport).
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(icon)
                            .child(div().text_sm().truncate().child(entry.item().label.clone()))
                            .with_animation(
                                ElementId::Name(format!("row-{id}").into()),
                                Animation::new(Duration::from_millis(150)),
                                |row, delta| row.opacity(delta),
                            ),
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
            .child(self.render_title_bar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .p_4()
                    .pt_2()
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
                    .child(self.render_content(cx)),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
