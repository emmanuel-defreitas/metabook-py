//! Main application view.
//!
//! A Sketch-aligned library and metadata workspace. A shared search toolbar
//! and EPUB intake sit beside a content region that swaps
//! to the processing, disambiguation, result, and failure views as a request
//! progresses.
//!
//! State ownership: `MetabookApp` owns the workflow phase, the form states,
//! the library filter, and the persisted library. Async requests carry a
//! request index so a stale response can never overwrite a newer one.

mod components;
mod detail;
mod graph;
mod graph_data;
mod helpers;
mod home;
mod loading;
mod methods;
mod styles;

use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use ely_gpui_component::forms::TextInput;
use ely_gpui_component::layout::AppShell;
use ely_gpui_component::primitives::FocusScope;
use ely_gpui_component::shell::TitleBar;
use ely_gpui_component::theme::{ActiveTheme as _, Mode, Radius, TextSize, Theme as ElyTheme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, radians, AnyElement, AppContext as _, ClipboardItem, Context, ElementId, Entity,
    HighlightStyle, Image, ImageFormat, InteractiveElement as _, IntoElement, ParentElement,
    PathPromptOptions, Render, SharedString, StatefulInteractiveElement as _, Styled, Subscription,
    Window,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{EditorState, Position, TextDecoration};
use gpui_component::list::ListItem;
use gpui_component::select::SelectState;
use gpui_component::spinner::Spinner;
use gpui_component::tree::{tree, TreeEvent, TreeState};
use gpui_component::{h_flex, v_flex, Icon, IconName, Root, Sizable as _};
use gpui_motion::{MotionExt as _, Spring, Tween};

use crate::api::{self, LibraryBook, NodeSpan, SearchOutcome, SearchPage, TreeNode};
use helpers::{materialize_items, META_SEPARATOR};
use styles::{DETAIL_OPTIONS, TOKENIZER_DEFAULT_IX, TOKENIZER_OPTIONS};

/// The workflow phase shown in the content region.
enum Phase {
    Idle,
    Processing {
        message: SharedString,
        detail_loading: bool,
        home_view: bool,
    },
    /// One page of Gutendex results; selection starts analysis.
    Matches {
        page: SearchPage,
    },
    Done {
        title: SharedString,
        value: Rc<serde_json::Value>,
        schema_json: SharedString,
        /// Node id → location of that node in the JSON document.
        ranges: HashMap<String, NodeSpan>,
        /// Node id currently synced to the editor.
        selected_node: Option<String>,
        graph_focus: graph_data::Focus,
        graph_page: usize,
        /// Source tree; `TreeItem`s are materialised lazily from this as the
        /// user expands folders, so huge trees cost O(visible), not O(total).
        tree: Rc<Vec<TreeNode>>,
        /// Ids currently expanded in the tree.
        expanded: HashSet<SharedString>,
        tree_state: Entity<TreeState>,
        /// Read-only JSON code editor (tree-sitter highlighting, folding).
        /// `None` while it initialises one frame after the result arrives —
        /// a skeleton shows in its place so the tree is usable immediately.
        editor_state: Option<Entity<EditorState>>,
        decorations: Option<gpui_component::input::TextDecorationCollection>,
    },
    Failed {
        message: SharedString,
    },
}

/// The persisted library shown on the library (`GET /api/books/uploads`):
/// every book uploaded to Vercel Blob or selected from search results, kept
/// by the API with its scan state.
enum Library {
    Loading,
    Ready(Vec<LibraryBook>),
    Failed(SharedString),
}

/// A book cover, fetched once per URL and shared by every card that shows it.
enum Cover {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

/// The image format for downloaded bytes, from their magic number. GPUI needs
/// the format up front, and a wrong guess renders nothing.
fn image_format(bytes: &[u8]) -> Option<ImageFormat> {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Some(ImageFormat::Jpeg),
        [0x89, b'P', b'N', b'G', ..] => Some(ImageFormat::Png),
        [b'G', b'I', b'F', ..] => Some(ImageFormat::Gif),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageFormat::Webp),
        _ => None,
    }
}

pub struct MetabookApp {
    focus: gpui::FocusHandle,
    api_base: SharedString,
    full_json: bool,
    query: Entity<TextInput>,
    upload_progress: Option<Arc<api::UploadProgress>>,
    tokenizer: Entity<SelectState<Vec<&'static str>>>,
    detail: Entity<SelectState<Vec<&'static str>>>,
    epub_path: Option<PathBuf>,
    phase: Phase,
    library: Library,
    covers: HashMap<SharedString, Cover>,
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
        // (e.g. a container runtime) listens on *:8001, localhost can resolve
        // to ::1 and reach that service instead of the local API.
        let api_base = std::env::var("METABOOK_API")
            .unwrap_or_else(|_| "http://127.0.0.1:8001".into())
            .trim_end_matches('/')
            .to_string();

        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let query = cx.new(|cx| {
            TextInput::new(window, cx).placeholder("Search by title, author, ISBN, or Gutenberg ID")
        });
        // Optional token counting: a known Hugging Face tokenizer, defaulting
        // to bert-base-uncased. "No tokens" omits the parameter entirely.
        let tokenizer = cx.new(|cx| {
            SelectState::new(
                TOKENIZER_OPTIONS.to_vec(),
                Some(gpui_component::IndexPath::new(TOKENIZER_DEFAULT_IX)),
                window,
                cx,
            )
        });
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

        let subscriptions = vec![cx.subscribe_in(&query, window, Self::on_input_event)];

        let mut app = Self {
            focus,
            api_base: api_base.into(),
            full_json: false,
            query,
            upload_progress: None,
            tokenizer,
            detail,
            epub_path: None,
            phase: Phase::Idle,
            library: Library::Loading,
            covers: HashMap::new(),
            request_ix: 0,
            copied: false,
            expand_gen: 0,
            last_expanded: None,
            _subscriptions: subscriptions,
        };
        app.refresh_library(cx);
        app
    }

    // ── Commands ───────────────────────────────────────────────────────────────

    fn start_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let query = self.query.read(cx).text().trim().to_string();
        if query.is_empty() {
            self.phase = Phase::Failed {
                message: "Enter a title, an ISBN, or a Gutenberg ID to search.".into(),
            };
            cx.notify();
            return;
        }

        self.search_page(query, 1, window, cx);
    }

    fn search_page(
        &mut self,
        query: String,
        page: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_processing() {
            return;
        }
        let base = self.api_base.to_string();
        self.begin_request(
            "Searching Gutendex…",
            false,
            true,
            move || api::search(&base, &query, page),
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
        let tokenizer = self.tokenizer_value(cx);
        self.begin_request(
            "Fetching and scanning the book text…",
            true,
            false,
            move || {
                api::fetch_by_id(&base, gutenberg_id, &detail, &tokenizer)
                    .map(SearchOutcome::Analysis)
            },
            window,
            cx,
        );
    }

    fn select_upload(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let base = self.api_base.to_string();
        self.begin_request(
            "Opening the saved book structure…",
            true,
            false,
            move || api::fetch_upload(&base, &id).map(SearchOutcome::Analysis),
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
        let tokenizer = self.tokenizer_value(cx);
        let progress = Arc::new(api::UploadProgress::default());
        self.upload_progress = Some(progress.clone());
        self.begin_request(
            "Uploading and scanning the EPUB…",
            false,
            true,
            move || {
                api::upload(&base, &path, &detail, &tokenizer, progress)
                    .map(SearchOutcome::Analysis)
            },
            window,
            cx,
        );
    }

    fn begin_request(
        &mut self,
        message: &'static str,
        detail_loading: bool,
        home_view: bool,
        work: impl FnOnce() -> Result<SearchOutcome, String> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_ix += 1;
        let ix = self.request_ix;
        self.phase = Phase::Processing {
            message: message.into(),
            detail_loading,
            home_view,
        };
        self.query
            .update(cx, |query, cx| query.set_disabled(true, cx));
        if self.upload_progress.is_some() && home_view {
            cx.spawn_in(window, async move |this, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let running = this
                    .update_in(cx, |this, _, cx| {
                        let running = this.request_ix == ix && this.is_processing();
                        if running {
                            cx.notify();
                        }
                        running
                    })
                    .unwrap_or(false);
                if !running {
                    break;
                }
            })
            .detach();
        }
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { work() }).await;
            this.update_in(cx, |this, window, cx| {
                this.finish_request(ix, result, window, cx)
            })
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
        self.upload_progress = None;
        self.query
            .update(cx, |query, cx| query.set_disabled(false, cx));
        self.phase = match result {
            Ok(SearchOutcome::Analysis(analysis)) => {
                let tree = Rc::new(analysis.tree);
                // Everything starts collapsed.
                let expanded: HashSet<SharedString> = HashSet::new();
                let items = materialize_items(&tree, &expanded);
                let tree_state = cx.new(|cx| TreeState::new(cx).items(items));
                self.full_json = false;
                self.expand_gen = 0;
                self.last_expanded = None;
                // No selection event exists; observe the state and react to
                // whatever entry is selected after each change. Expansions
                // emit events, which both materialise the newly revealed
                // children and drive the row entrance animation.
                cx.observe_in(&tree_state, window, Self::on_tree_changed)
                    .detach();
                cx.subscribe(&tree_state, |this, _, event: &TreeEvent, cx| {
                    this.on_tree_toggle(event, cx);
                })
                .detach();

                // Defer the editor: building a rope from a many-megabyte JSON
                // string blocks the main thread, so paint the result frame
                // (with a skeleton in the JSON pane) first.
                let schema_json = SharedString::from(analysis.schema_json);
                cx.spawn_in(window, {
                    let schema_json = schema_json.clone();
                    async move |this, cx| {
                        this.update_in(cx, |this, window, cx| {
                            this.init_editor(schema_json, window, cx)
                        })
                        .ok();
                    }
                })
                .detach();

                Phase::Done {
                    title: analysis.title.into(),
                    value: Rc::new(analysis.value),
                    schema_json,
                    ranges: analysis.ranges,
                    selected_node: None,
                    graph_focus: graph_data::Focus::Book,
                    graph_page: 0,
                    tree,
                    expanded,
                    tree_state,
                    editor_state: None,
                    decorations: None,
                }
            }
            Ok(SearchOutcome::Matches(page)) => Phase::Matches { page },
            Err(message) => Phase::Failed {
                message: message.into(),
            },
        };
        // A finished analysis is persisted by the API, so the library list
        // has a new book to show.
        if matches!(self.phase, Phase::Done { .. }) {
            self.refresh_library(cx);
        }
        cx.notify();
    }

    fn choose_epub(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(path) = paths.into_iter().next() {
                    this.update_in(cx, |this, window, cx| {
                        this.set_epub_path(path, cx);
                        if !matches!(this.phase, Phase::Failed { .. }) {
                            this.start_upload(window, cx);
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn set_epub_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("epub"))
        {
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

    /// One frame after a result arrives, build the JSON editor behind the
    /// skeleton and swap it in.
    fn init_editor(
        &mut self,
        schema_json: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Phase::Done {
            editor_state,
            decorations,
            ..
        } = &mut self.phase
        else {
            return;
        };
        if editor_state.is_some() {
            return;
        }
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .folding(true)
                .default_value(schema_json)
        });
        let collection = state.update(cx, |state, cx| {
            state.set_readonly(true, cx);
            state.create_decorations_collection(vec![], cx)
        });
        *editor_state = Some(state);
        *decorations = Some(collection);
        cx.notify();
    }

    /// Materialise the children of a folder the first time it expands and
    /// keep the expansion set in sync.
    fn on_tree_toggle(&mut self, event: &TreeEvent, cx: &mut Context<Self>) {
        let Phase::Done {
            tree,
            expanded,
            tree_state,
            ..
        } = &mut self.phase
        else {
            return;
        };
        let changed = match event {
            TreeEvent::Expanded(id) => {
                self.last_expanded = Some(id.clone());
                self.expand_gen += 1;
                expanded.insert(id.clone())
            }
            TreeEvent::Collapsed(id) => expanded.remove(id),
        };
        if changed {
            let items = materialize_items(tree, expanded);
            let selected = tree_state.read(cx).selected_index();
            tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
                state.set_selected_index(selected, cx);
            });
            cx.notify();
        }
    }

    /// Collapse every folder in the tree at once.
    fn collapse_all(&mut self, cx: &mut Context<Self>) {
        let Phase::Done {
            tree,
            expanded,
            tree_state,
            ..
        } = &mut self.phase
        else {
            return;
        };
        if expanded.is_empty() {
            return;
        }
        expanded.clear();
        self.last_expanded = None;
        let items = materialize_items(tree, expanded);
        tree_state.update(cx, |state, cx| state.set_items(items, cx));
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
        self.select_structure_node(selected_id, window, cx);
    }

    fn select_structure_node(
        &mut self,
        selected_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let highlight_bg = cx.theme().colors.selection;
        let Phase::Done {
            ranges,
            selected_node,
            graph_focus,
            graph_page,
            editor_state,
            decorations,
            ..
        } = &mut self.phase
        else {
            return;
        };
        if *selected_node == selected_id {
            return;
        }
        *selected_node = selected_id.clone();
        *graph_focus = graph_data::Focus::Structure(selected_id.clone());
        *graph_page = 0;
        cx.notify();
        let (Some(editor_state), Some(decorations)) = (editor_state.clone(), decorations.clone())
        else {
            return;
        };
        let span = selected_id.and_then(|id| ranges.get(&id).cloned());
        if let Some(span) = span {
            editor_state.update(cx, |state, cx| {
                // The cursor stops at a fold boundary if the span is inside
                // one; unfold just the folds containing the span first.
                let position = Position::new(span.line as u32, 0);
                state.unfold_at(position, cx);
                state.set_cursor_position(position, window, cx);
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

    /// Reload the persisted library in the background.
    ///
    /// A library that already has books keeps them on screen while the fetch
    /// runs (no skeleton flash), and a failed refresh only surfaces when
    /// there is nothing to preserve.
    fn refresh_library(&mut self, cx: &mut Context<Self>) {
        let had_books = matches!(self.library, Library::Ready(_));
        if !had_books {
            self.library = Library::Loading;
        }
        cx.notify();

        let base = self.api_base.to_string();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { api::list_uploads(&base) })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(books) => {
                        this.library = Library::Ready(books);
                        this.load_covers(cx);
                    }
                    Err(message) => {
                        if !had_books {
                            this.library = Library::Failed(message.into());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Fetch the covers this library needs and hasn't tried yet.
    ///
    /// Keyed by URL, so refreshing the library re-uses every cover already in
    /// hand and only the genuinely new books hit the network.
    fn load_covers(&mut self, cx: &mut Context<Self>) {
        let Library::Ready(books) = &self.library else {
            return;
        };
        let pending: Vec<SharedString> = books
            .iter()
            .filter_map(|book| book.cover_url.clone())
            .map(SharedString::from)
            .filter(|url| !self.covers.contains_key(url))
            .collect();

        for url in pending {
            self.covers.insert(url.clone(), Cover::Loading);
            cx.spawn(async move |this, cx| {
                let request_url = url.to_string();
                let result = cx
                    .background_spawn(async move { api::fetch_cover(&request_url) })
                    .await;
                this.update(cx, |this, cx| {
                    let cover = match result {
                        Ok(bytes) => match image_format(&bytes) {
                            Some(format) => {
                                Cover::Ready(Arc::new(Image::from_bytes(format, bytes)))
                            }
                            None => Cover::Failed,
                        },
                        Err(_) => Cover::Failed,
                    };
                    this.covers.insert(url, cover);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Sidebar navigation: leave a result, match list, or failure behind and
    /// return to the library. The form inputs and the chosen EPUB survive.
    fn show_dashboard(&mut self, cx: &mut Context<Self>) {
        if self.is_processing() || matches!(self.phase, Phase::Idle) {
            return;
        }
        self.phase = Phase::Idle;
        cx.notify();
    }

    fn toggle_theme(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let mode = if cx.theme().is_dark() {
            Mode::Light
        } else {
            Mode::Dark
        };
        ElyTheme::set_mode(mode, cx);
    }

    // ── Regions ────────────────────────────────────────────────────────────────

    fn render_title_bar(&self, _cx: &Context<Self>) -> impl IntoElement {
        TitleBar::new("title-bar")
    }

    /// The work area. Idle shows the library (its own scroll owner, so the
    /// scrollbar sits at the panel edge); every other phase is a page inside
    /// the shared content inset.
    fn render_content(&self, cx: &Context<Self>) -> AnyElement {
        match &self.phase {
            Phase::Idle => self.render_home(cx),
            Phase::Processing {
                home_view: true, ..
            } => self.render_home(cx),
            Phase::Processing {
                message,
                detail_loading: true,
                ..
            } => self.render_detail_loading(message.clone(), cx),
            Phase::Processing { message, .. } => {
                Self::render_page(self.render_processing(message.clone(), cx))
            }
            Phase::Matches { page } => Self::render_page(self.render_matches(page.clone(), cx)),
            Phase::Failed { message } => Self::render_page(self.render_failed(message.clone(), cx)),
            Phase::Done { .. } => self.render_book_detail(cx),
        }
    }

    /// The shared content inset every non-library page sits in.
    fn render_page(inner: impl IntoElement) -> AnyElement {
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .p_4()
            .pt_2()
            .child(inner)
            .into_any_element()
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
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(
                        "Book metadata and structural counts are returned without the book text.",
                    ),
            )
    }

    fn render_matches(&self, page: SearchPage, cx: &Context<Self>) -> impl IntoElement {
        let count = page.count;
        let page_number = page.page;
        let previous_query = page.query.clone();
        let next_query = page.query.clone();
        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .text_color(cx.theme().colors.fg_muted)
                    .child(if count == 0 {
                        "No books found in Gutendex. Try a title, author, or Gutenberg ID. ISBN lookup is a keyword search.".into()
                    } else {
                        let plural = if count == 1 { "" } else { "s" };
                        format!("{count} Gutendex result{plural} · Page {page_number} — select a book to analyse")
                    }),
            )
            .child(
                v_flex()
                    .id("match-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(page.matches.into_iter().map(|book| {
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
                            .bg(cx.theme().colors.surface)
                            .border_color(cx.theme().colors.border)
                            .rounded(cx.theme().radius(Radius::Lg))
                            .hover(|style| style.bg(cx.theme().colors.hover))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .min_w_0()
                                    .child(div().truncate().child(book.title))
                                    .child(
                                        div()
                                            .text_size(cx.theme().text_size(TextSize::Sm))
                                            .text_color(cx.theme().colors.fg_muted)
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
                                            .text_size(cx.theme().text_size(TextSize::Sm))
                                            .text_color(cx.theme().colors.fg_muted)
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
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_match(id, window, cx)
                            }))
                    })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .when_some(page.previous_page, |row, previous| {
                        row.child(Button::new("search-previous").small().label("Previous").on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.search_page(previous_query.clone(), previous, window, cx)
                            }),
                        ))
                    })
                    .when_some(page.next_page, |row, next| {
                        row.child(Button::new("search-next").small().label("Next").on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.search_page(next_query.clone(), next, window, cx)
                            }),
                        ))
                    }),
            )
    }

    fn render_failed(&self, message: SharedString, cx: &Context<Self>) -> impl IntoElement {
        use ely_gpui_component::buttons::{Button as ElyButton, ButtonVariant};
        v_flex().size_full().items_center().justify_center().child(
            ely_gpui_component::feedback::ResultView::failure(
                ("request-failed", self.request_ix),
                "Unable to complete the book request",
            )
            .body(message)
            .action(
                ElyButton::new("failure-library", "Return to library")
                    .variant(ButtonVariant::Outline)
                    .on_click(cx.listener(|this, _, _, cx| this.show_dashboard(cx))),
            ),
        )
    }

    fn render_structure_tree(
        &self,
        tree_state: Entity<TreeState>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .pr_3()
            .child(
                h_flex().justify_end().pb_1().child(
                    Button::new("collapse-all")
                        .ghost()
                        .small()
                        .icon(IconName::ChevronsUpDown)
                        .label("Collapse all")
                        .on_click(cx.listener(|this, _, _, cx| this.collapse_all(cx))),
                ),
            )
            .child(div().flex_1().min_h_0().child(tree(&tree_state, {
                let expand_gen = self.expand_gen;
                let last_expanded = self.last_expanded.clone();
                move |ix, entry, selected, _, cx| {
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

                    // The materialised label carries the node's counts behind
                    // META_SEPARATOR ("Paragraph 1␟2 sentences · 24 words ·
                    // 31 tokens"): the name truncates while the counts render
                    // as muted text that never shrinks, so token counts
                    // survive a narrow panel.
                    let label = entry.item().label.clone();
                    let (name, counts) = match label.split_once(META_SEPARATOR) {
                        Some((name, counts)) => (name.to_string(), Some(counts.to_string())),
                        None => (label.to_string(), None),
                    };
                    let content = h_flex()
                        .gap_2()
                        .items_center()
                        .child(icon)
                        .child(
                            div()
                                .text_size(cx.theme().text_size(TextSize::Sm))
                                .truncate()
                                .child(name),
                        )
                        .when_some(counts, |row, counts| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_size(cx.theme().text_size(TextSize::Xs))
                                    .text_color(cx.theme().colors.fg_muted)
                                    .child(counts),
                            )
                        });

                    // Only rows revealed by the latest expansion animate in;
                    // everything else renders statically (scrolling never
                    // replays an entrance animation).
                    let just_revealed = last_expanded.as_ref().is_some_and(|parent| {
                        id.starts_with(&format!("{parent}.")) && id.as_ref() != parent.as_ref()
                    });

                    let item = ListItem::new(ix)
                        .selected(selected)
                        .pl(px(16.) * entry.depth() as f32 + px(4.));
                    if just_revealed {
                        item.child(
                            content
                                .with_motion(
                                    ElementId::Name(format!("reveal-{expand_gen}-{id}").into()),
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
            })))
    }
}

impl Render for MetabookApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::theme::synchronize(cx);
        let content = v_flex()
            .size_full()
            .font_family(cx.theme().font_family.clone())
            .text_size(cx.theme().text_size(TextSize::Base))
            .text_color(cx.theme().colors.fg)
            .child(
                AppShell::new()
                    .title_bar(
                        v_flex()
                            .child(self.render_title_bar(cx))
                            .when(!self.is_home(), |bar| bar.child(self.render_toolbar(cx))),
                    )
                    .when(
                        matches!(
                            self.phase,
                            Phase::Done { .. }
                                | Phase::Processing {
                                    detail_loading: true,
                                    ..
                                }
                        ),
                        |shell| shell.sidebar(self.render_sidebar(cx)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .text_color(cx.theme().colors.fg)
                            .child(self.render_content(cx)),
                    ),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx));
        FocusScope::new(&self.focus)
            .root()
            .size_full()
            .child(content)
    }
}
