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
use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, BorrowAppContext as _, ClipboardItem, Context, ElementId,
    Entity, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, px, radians, relative,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{
    Editor, EditorState, Input, InputEvent, InputState, Position, TextDecoration,
};
use gpui_component::list::ListItem;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::select::{Select, SelectState};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::tree::{TreeEvent, TreeItem, TreeState, tree};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Root, Sizable as _, StyledExt as _,
    Theme, ThemeMode, TitleBar, h_flex, v_flex,
};
use gpui_motion::{MotionExt as _, Spring, Tween};
use gpui_navigator::{GlobalRouter, Transition as RouteTransition, router_view};

use crate::api::{self, BookMatch, NodeSpan, SearchOutcome, TreeNode};

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
        /// Node id → location of that node in the JSON document.
        ranges: HashMap<String, NodeSpan>,
        /// Node id currently synced to the editor.
        selected_node: Option<String>,
        tree_state: Entity<TreeState>,
        /// Read-only JSON code editor (tree-sitter highlighting, folding).
        editor_state: Entity<EditorState>,
        decorations: gpui_component::input::TextDecorationCollection,
    },
    Failed { message: SharedString },
}

/// The little bit of dynamic state the route pages need. Kept in its own
/// entity so the router outlet can read it while `MetabookApp` itself is
/// mid-render (reading the app entity there would be re-entrant).
pub struct FormFlags {
    processing: bool,
    epub_name: Option<SharedString>,
}

/// Everything a route page needs, captured once at router setup.
#[derive(Clone)]
pub struct FormHandles {
    pub app: gpui::WeakEntity<MetabookApp>,
    query: Entity<InputState>,
    isbn: Entity<InputState>,
    detail: Entity<SelectState<Vec<&'static str>>>,
    flags: Entity<FormFlags>,
}

pub struct MetabookApp {
    api_base: SharedString,
    tab_ix: usize,
    query: Entity<InputState>,
    isbn: Entity<InputState>,
    detail: Entity<SelectState<Vec<&'static str>>>,
    flags: Entity<FormFlags>,
    epub_path: Option<PathBuf>,
    phase: Phase,
    /// Incremented per request; responses for an older index are discarded.
    request_ix: usize,
    /// True briefly after Copy JSON, driving the button's success feedback.
    copied: bool,
    /// Bumped on every tree expansion; keys the entrance animation so only
    /// the most recently revealed rows animate (no flashing on scroll).
    expand_gen: u64,
    /// The folder id expanded most recently.
    last_expanded: Option<SharedString>,
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

        let flags = cx.new(|_| FormFlags { processing: false, epub_name: None });

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
            flags,
            epub_path: None,
            phase: Phase::Idle,
            request_ix: 0,
            copied: false,
            expand_gen: 0,
            last_expanded: None,
            _subscriptions: subscriptions,
        }
    }

    fn on_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::PressEnter { .. } = event {
            self.start_search(window, cx);
        }
    }

    /// Handles for the router's page builders.
    pub fn form_handles(&self, cx: &Context<Self>) -> FormHandles {
        FormHandles {
            app: cx.entity().downgrade(),
            query: self.query.clone(),
            isbn: self.isbn.clone(),
            detail: self.detail.clone(),
            flags: self.flags.clone(),
        }
    }

    fn set_flags(&mut self, cx: &mut Context<Self>) {
        let processing = self.is_processing();
        let epub_name: Option<SharedString> = self.epub_path.as_ref().map(|path| {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
                .into()
        });
        self.flags.update(cx, |flags, _| {
            flags.processing = processing;
            flags.epub_name = epub_name;
        });
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

    fn start_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            window,
            cx,
        );
    }

    fn select_match(&mut self, gutenberg_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let base = self.api_base.to_string();
        let detail = self.detail_value(cx);
        self.begin_request(
            "Fetching and scanning the book text…",
            move || api::fetch_by_id(&base, gutenberg_id, &detail).map(SearchOutcome::Analysis),
            window,
            cx,
        );
    }

    fn start_upload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            window,
            cx,
        );
    }

    fn begin_request(
        &mut self,
        message: &'static str,
        work: impl FnOnce() -> Result<SearchOutcome, String> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_ix += 1;
        let ix = self.request_ix;
        self.phase = Phase::Processing { message: message.into() };
        self.set_flags(cx);
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { work() }).await;
            this.update_in(cx, |this, window, cx| this.finish_request(ix, result, window, cx))
                .ok();
        })
        .detach();
    }

    fn finish_request(
        &mut self,
        ix: usize,
        result: Result<SearchOutcome, String>,
        window: &mut Window,
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
                self.expand_gen = 0;
                self.last_expanded = None;
                // No selection event exists; observe the state and react to
                // whatever entry is selected after each change. Expansions do
                // emit events, which drive the row entrance animation.
                cx.observe_in(&tree_state, window, Self::on_tree_changed).detach();
                cx.subscribe(&tree_state, |this, _, event: &TreeEvent, cx| {
                    if let TreeEvent::Expanded(id) = event {
                        this.expand_gen += 1;
                        this.last_expanded = Some(id.clone());
                        cx.notify();
                    }
                })
                .detach();

                let editor_state = cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language("json")
                        .line_number(true)
                        .folding(true)
                        .default_value(SharedString::from(analysis.schema_json.clone()))
                });
                let decorations = editor_state.update(cx, |state, cx| {
                    state.set_readonly(true, cx);
                    state.create_decorations_collection(vec![], cx)
                });

                Phase::Done {
                    title: analysis.title.into(),
                    schema_json: analysis.schema_json.into(),
                    ranges: analysis.ranges,
                    selected_node: None,
                    tree_state,
                    editor_state,
                    decorations,
                }
            }
            Ok(SearchOutcome::Matches(matches)) => Phase::Matches { matches },
            Err(message) => Phase::Failed { message: message.into() },
        };
        self.set_flags(cx);
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
        self.set_flags(cx);
        cx.notify();
    }

    /// After any tree change, sync the JSON editor to the selected node:
    /// move the cursor to its first line (scrolling it into view) and
    /// decorate its byte range with a highlight.
    fn on_tree_changed(
        &mut self,
        state: Entity<TreeState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected_id = state
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.to_string());
        let highlight_bg = cx.theme().selection;
        let Phase::Done { ranges, selected_node, editor_state, decorations, .. } =
            &mut self.phase
        else {
            return;
        };
        if *selected_node == selected_id {
            return;
        }
        *selected_node = selected_id.clone();
        let span = selected_id.and_then(|id| ranges.get(&id).cloned());
        let editor_state = editor_state.clone();
        let decorations = decorations.clone();
        if let Some(span) = span {
            editor_state.update(cx, |state, cx| {
                // gpui-component has no public targeted unfold, and the cursor
                // stops at a fold boundary if the span is inside one; cycling
                // folding off/on clears all folds (candidates survive) so the
                // cursor can reach the span. Once longbridge/gpui-component#2872
                // (unfold_ranges_containing) lands, unfold just span.bytes.start.
                state.set_folding(false, window, cx);
                state.set_folding(true, window, cx);
                state.set_cursor_position(Position::new(span.line as u32, 0), window, cx);
            });
            decorations.set(
                vec![TextDecoration::new(
                    span.bytes,
                    HighlightStyle {
                        background_color: Some(highlight_bg),
                        ..Default::default()
                    },
                )],
                cx,
            );
        } else {
            decorations.set(Vec::new(), cx);
        }
        cx.notify();
    }

    fn copy_schema(&mut self, cx: &mut Context<Self>) {
        if let Phase::Done { schema_json, .. } = &self.phase {
            cx.write_to_clipboard(ClipboardItem::new_string(schema_json.to_string()));
            self.copied = true;
            cx.notify();
            // Revert the button's success state after a beat.
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1400))
                    .await;
                this.update(cx, |this, cx| {
                    this.copied = false;
                    cx.notify();
                })
                .ok();
            })
            .detach();
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

    // ── Route pages ────────────────────────────────────────────────────────────
    //
    // These build the search/upload forms from outside the entity's own
    // render pass (the router outlet calls them while `MetabookApp` renders),
    // so they read state via `Entity::read` and wire handlers with plain
    // closures that only `update` on interaction.

    pub(crate) fn search_form(handles: &FormHandles, cx: &mut App) -> AnyElement {
        let processing = handles.flags.read(cx).processing;
        let (query, isbn, detail) = (
            handles.query.clone(),
            handles.isbn.clone(),
            handles.detail.clone(),
        );
        h_flex()
            .gap_2()
            .items_center()
            .child(div().flex_1().child(Input::new(&query)))
            .child(div().w_48().child(Input::new(&isbn)))
            .child(div().w_40().child(Select::new(&detail)))
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
                            app.update(cx, |this, cx| this.start_search(window, cx)).ok();
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
            .child(div().w_40().child(Select::new(&handles.detail)))
            .child(
                Button::new("analyze")
                    .primary()
                    .icon(IconName::ArrowUp)
                    .label("Analyze")
                    .loading(processing)
                    .disabled(processing || !has_file)
                    .on_click({
                        let app = handles.app.clone();
                        move |_, window, cx| {
                            app.update(cx, |this, cx| this.start_upload(window, cx)).ok();
                        }
                    }),
            )
            .into_any_element()
    }

    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let content = match &self.phase {
            Phase::Idle => self.render_idle(cx).into_any_element(),
            Phase::Processing { message } => self.render_processing(message.clone(), cx).into_any_element(),
            Phase::Matches { matches } => self.render_matches(matches.clone(), cx).into_any_element(),
            Phase::Failed { message } => self.render_failed(message.clone(), cx).into_any_element(),
            Phase::Done { title, tree_state, editor_state, .. } => self
                .render_result(title.clone(), tree_state.clone(), editor_state.clone(), cx)
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
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.select_match(id, window, cx)
                                            })),
                                    ),
                            )
                            .on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.select_match(id, window, cx)
                                }),
                            )
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
        tree_state: Entity<TreeState>,
        editor_state: Entity<EditorState>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let copied = self.copied;
        let success = cx.theme().success;
        let copy_button = Button::new("copy-schema")
            .ghost()
            .small()
            .icon(if copied { IconName::Check } else { IconName::Copy })
            .label(if copied { "Copied" } else { "Copy JSON" })
            .on_click(cx.listener(|this, _, _, cx| this.copy_schema(cx)));
        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_semibold().child(title))
                    .child(
                        // Pop-in feedback: the id changes with `copied`, so a
                        // fresh spring runs on both copy and revert.
                        div()
                            .when(copied, |el| el.text_color(success))
                            .child(copy_button)
                            .with_motion(
                                ElementId::Name(format!("copy-fb-{copied}").into()),
                                1.0f32,
                                Spring::wobbly(),
                                |el, t: f32| el.opacity(0.4 + 0.6 * t),
                            )
                            .initial(0.0),
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
                                    .child(Editor::new(&editor_state).h(relative(1.))),
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
            .child(tree(&tree_state, {
                let expand_gen = self.expand_gen;
                let last_expanded = self.last_expanded.clone();
                move |ix, entry, selected, _, _| {
                    let id = entry.item().id.clone();
                    let expanded = entry.is_expanded();

                    // The chevron animates toward its rotation target, so it
                    // renders settled when rows scroll into view and only
                    // animates on an actual toggle — no flashing.
                    let icon = if entry.is_folder() {
                        let target = if expanded { 1.0f32 } else { 0.0 };
                        div()
                            .with_motion(
                                ElementId::Name(format!("chev-{id}").into()),
                                target,
                                Spring::from_duration(0.2),
                                |wrapper, t: f32| {
                                    wrapper.child(
                                        Icon::new(IconName::ChevronRight)
                                            .small()
                                            .rotate(radians(t * FRAC_PI_2)),
                                    )
                                },
                            )
                            .into_any_element()
                    } else {
                        Icon::new(IconName::File).small().into_any_element()
                    };

                    let content = h_flex()
                        .gap_2()
                        .items_center()
                        .child(icon)
                        .child(div().text_sm().truncate().child(entry.item().label.clone()));

                    // Only rows revealed by the latest expansion animate in;
                    // everything else renders statically (scrolling never
                    // replays an entrance animation).
                    let just_revealed = last_expanded
                        .as_ref()
                        .is_some_and(|parent| {
                            id.starts_with(&format!("{parent}.")) && id.as_ref() != parent.as_ref()
                        });

                    let item = ListItem::new(ix)
                        .selected(selected)
                        .pl(px(16.) * entry.depth() as f32 + px(4.));
                    if just_revealed {
                        item.child(
                            content
                                .with_motion(
                                    ElementId::Name(
                                        format!("reveal-{expand_gen}-{id}").into(),
                                    ),
                                    1.0f32,
                                    Tween::new(0.18),
                                    |row, t: f32| row.opacity(t),
                                )
                                .initial(0.0),
                        )
                    } else {
                        item.child(content)
                    }
                }
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
                                if this.tab_ix == *ix {
                                    return;
                                }
                                // Slide toward the tab being selected.
                                let (path, transition) = if *ix == TAB_UPLOAD {
                                    ("/upload", RouteTransition::slide_left(200))
                                } else {
                                    ("/", RouteTransition::slide_right(200))
                                };
                                this.tab_ix = *ix;
                                cx.update_global::<GlobalRouter, _>(|router, cx| {
                                    router.push_with_transition(path.to_string(), transition, cx);
                                });
                                cx.notify();
                            }))
                            .child(Tab::new().label("Search"))
                            .child(Tab::new().label("Upload EPUB")),
                    )
                    // The active form is a route; the router view animates
                    // transitions between / (search) and /upload.
                    .child(div().w_full().child(router_view(window, cx)))
                    .child(self.render_content(cx)),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
