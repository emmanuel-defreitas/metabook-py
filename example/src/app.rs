//! Main application view.
//!
//! A sidebar workspace: the Dashboard (search Gutenberg, drag-and-drop an
//! EPUB, and the persisted library grid) beside a content region that swaps
//! to the processing, disambiguation, result, and failure views as a request
//! progresses.
//!
//! State ownership: `MetabookApp` owns the workflow phase, the form states,
//! the sidebar collapse, and the persisted library. Async requests carry a
//! request index so a stale response can never overwrite a newer one.

mod components;
mod helpers;
mod methods;
mod styles;

use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, radians, relative, AnyElement, AppContext as _, ClipboardItem, Context, ElementId,
    Entity, ExternalPaths, HighlightStyle, Image, ImageFormat, InteractiveElement as _,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Editor, EditorState, InputState, Position, TextDecoration};
use gpui_component::list::ListItem;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::select::SelectState;
use gpui_component::sidebar::{
    Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem, SidebarToggleButton,
};
use gpui_component::skeleton::Skeleton;
use gpui_component::spinner::Spinner;
use gpui_component::tree::{tree, TreeEvent, TreeState};
use gpui_component::{
    h_flex, highlighter::LanguageRegistry, v_flex, ActiveTheme as _, Collapsible as _, Icon,
    IconName, Root, Sizable as _, StyledExt as _, Theme, ThemeMode, TitleBar,
};
use gpui_motion::{MotionExt as _, Spring, Tween};

use crate::api::{self, BookMatch, LibraryBook, NodeSpan, SearchOutcome, TreeNode};
use helpers::{materialize_items, META_SEPARATOR};
use styles::{DETAIL_OPTIONS, TOKENIZER_DEFAULT_IX, TOKENIZER_OPTIONS};

/// The workflow phase shown in the content region.
enum Phase {
    Idle,
    Processing {
        message: SharedString,
    },
    /// A search matched several books; the user picks one to analyse.
    Matches {
        matches: Vec<BookMatch>,
    },
    Done {
        title: SharedString,
        schema_json: SharedString,
        /// Node id → location of that node in the JSON document.
        ranges: HashMap<String, NodeSpan>,
        /// Node id currently synced to the editor.
        selected_node: Option<String>,
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

/// The persisted library shown on the dashboard (`GET /api/books/uploads`):
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
    api_base: SharedString,
    /// Collapsed icon-rail state of the sidebar.
    sidebar_collapsed: bool,
    query: Entity<InputState>,
    isbn: Entity<InputState>,
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

        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Title or author, e.g. Pride and Prejudice")
        });
        let isbn = cx.new(|cx| InputState::new(window, cx).placeholder("ISBN-10 or ISBN-13"));
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

        let subscriptions = vec![
            cx.subscribe_in(&query, window, Self::on_input_event),
            cx.subscribe_in(&isbn, window, Self::on_input_event),
        ];

        let mut app = Self {
            api_base: api_base.into(),
            sidebar_collapsed: false,
            query,
            isbn,
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
        let tokenizer = self.tokenizer_value(cx);
        self.begin_request(
            "Searching Gutendex…",
            move || api::search(&base, &query, &isbn, &detail, &tokenizer),
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
            move || {
                api::fetch_by_id(&base, gutenberg_id, &detail, &tokenizer)
                    .map(SearchOutcome::Analysis)
            },
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
        self.begin_request(
            "Uploading and scanning the EPUB…",
            move || api::upload(&base, &path, &detail, &tokenizer).map(SearchOutcome::Analysis),
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
        self.phase = Phase::Processing {
            message: message.into(),
        };
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
        self.phase = match result {
            Ok(SearchOutcome::Analysis(analysis)) => {
                let tree = Rc::new(analysis.tree);
                // Everything starts collapsed.
                let expanded: HashSet<SharedString> = HashSet::new();
                let items = materialize_items(&tree, &expanded);
                let tree_state = cx.new(|cx| TreeState::new(cx).items(items));
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
                    schema_json,
                    ranges: analysis.ranges,
                    selected_node: None,
                    tree,
                    expanded,
                    tree_state,
                    editor_state: None,
                    decorations: None,
                }
            }
            Ok(SearchOutcome::Matches(matches)) => Phase::Matches { matches },
            Err(message) => Phase::Failed {
                message: message.into(),
            },
        };
        // A finished analysis is persisted by the API, so the library grid
        // has a new book to show.
        if matches!(self.phase, Phase::Done { .. }) {
            self.refresh_library(cx);
        }
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
                    this.update(cx, |this, cx| this.set_epub_path(path, cx))
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
        let highlight_bg = cx.theme().selection;
        let Phase::Done {
            ranges,
            selected_node,
            editor_state,
            decorations,
            ..
        } = &mut self.phase
        else {
            return;
        };
        let (Some(editor_state), Some(decorations)) = (editor_state.clone(), decorations.clone())
        else {
            return;
        };
        if *selected_node == selected_id {
            return;
        }
        *selected_node = selected_id.clone();
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
    /// return to the dashboard. The form inputs and the chosen EPUB survive.
    fn show_dashboard(&mut self, cx: &mut Context<Self>) {
        if self.is_processing() || matches!(self.phase, Phase::Idle) {
            return;
        }
        self.phase = Phase::Idle;
        cx.notify();
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    /// Files dragged from the operating system onto the dashboard drop zone.
    /// The first `.epub` wins; anything else reports what the zone accepts.
    fn on_epub_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let epub = paths
            .0
            .iter()
            .find(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("epub"))
            })
            .cloned();
        match epub {
            Some(path) => self.set_epub_path(path, cx),
            None => {
                self.phase = Phase::Failed {
                    message: "Drop an .epub file — other formats can't be scanned.".into(),
                };
                cx.notify();
            }
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

    /// Custom title bar: window chrome only — the app identity lives in the
    /// sidebar header, so this keeps just the appearance toggle beside the
    /// drag area. Transparent with no bottom border, so the window reads as
    /// one surface with the content below.
    fn render_title_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme_icon = if cx.theme().is_dark() {
            IconName::Sun
        } else {
            IconName::Moon
        };
        TitleBar::new()
            .bg(cx.theme().transparent)
            .border_b_0()
            .child(
                h_flex().w_full().items_center().justify_end().pr_2().child(
                    Button::new("toggle-theme")
                        .ghost()
                        .small()
                        .icon(theme_icon)
                        .tooltip("Switch between light and dark mode")
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
                ),
            )
    }

    /// Persistent navigation beside the work area. The sidebar owns the app
    /// identity and the Dashboard destination; collapsing it leaves an icon
    /// rail so a narrow window keeps the full content width.
    fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;
        let on_dashboard = matches!(self.phase, Phase::Idle);

        Sidebar::new("app-sidebar")
            .collapsed(collapsed)
            .header(
                SidebarHeader::new().collapsed(collapsed).child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .min_w_0()
                                .child(Icon::new(IconName::BookOpen).small())
                                .when(!collapsed, |row| {
                                    row.child(
                                        div()
                                            .text_sm()
                                            .font_semibold()
                                            .truncate()
                                            .child("Metabook"),
                                    )
                                }),
                        )
                        .child(
                            SidebarToggleButton::new()
                                .collapsed(collapsed)
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Library").child(
                    SidebarMenu::new().child(
                        SidebarMenuItem::new("Dashboard")
                            .icon(IconName::LayoutDashboard)
                            .active(on_dashboard)
                            .on_click(cx.listener(|this, _, _, cx| this.show_dashboard(cx))),
                    ),
                ),
            )
    }

    /// The at-a-glance state for the status bar. The content region carries
    /// the full message; this stays a short state word.
    fn status_label(&self) -> SharedString {
        match &self.phase {
            Phase::Idle => "Ready".into(),
            Phase::Processing { .. } => "Scanning…".into(),
            Phase::Matches { .. } => "Select a match".into(),
            Phase::Done { .. } => "Schema ready".into(),
            Phase::Failed { .. } => "Failed".into(),
        }
    }

    /// Bottom status band: which API instance the app talks to and the
    /// workflow state on the leading side, the appearance mode and the JSON
    /// highlighting engine on the trailing side.
    ///
    /// The band uses the `title_bar` chrome-surface token because the plain
    /// `border` hairline is token-identical to the secondary window surface
    /// in both themes — a distinct chrome band keeps the boundary readable
    /// in light and dark alike.
    fn render_status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let failed = matches!(self.phase, Phase::Failed { .. });
        // The JSON pane is highlighted through tree-sitter when the grammar
        // is registered; without it the editor falls back to plain text.
        let json_highlighted = LanguageRegistry::singleton().language("json").is_some();
        let (json_icon, json_label) = if json_highlighted {
            (IconName::CircleCheck, "JSON · tree-sitter")
        } else {
            (IconName::TriangleAlert, "JSON · plain text")
        };
        let theme_label: &str = if cx.theme().is_dark() {
            "Dark"
        } else {
            "Light"
        };

        h_flex()
            .id("status-bar")
            .w_full()
            .flex_none()
            .h_10()
            .items_center()
            .justify_between()
            .px_3()
            .bg(cx.theme().title_bar)
            .border_t_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .min_w_0()
                    .child(Icon::new(IconName::Globe).xsmall())
                    .child(div().truncate().child(self.api_base.clone()))
                    .child(div().child("·"))
                    .child(
                        div()
                            .truncate()
                            .when(failed, |el| el.text_color(cx.theme().danger))
                            .child(self.status_label()),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .flex_none()
                    .child(div().child(theme_label))
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(Icon::new(json_icon).xsmall())
                            .child(div().child(json_label)),
                    ),
            )
    }

    /// The work area. Idle shows the dashboard (its own scroll owner, so the
    /// scrollbar sits at the panel edge); every other phase is a page inside
    /// the shared content inset.
    fn render_content(&self, cx: &Context<Self>) -> AnyElement {
        match &self.phase {
            Phase::Idle => self.render_dashboard(cx),
            Phase::Processing { message } => {
                Self::render_page(self.render_processing(message.clone(), cx))
            }
            Phase::Matches { matches } => {
                Self::render_page(self.render_matches(matches.clone(), cx))
            }
            Phase::Failed { message } => Self::render_page(self.render_failed(message.clone(), cx)),
            Phase::Done {
                title,
                tree_state,
                editor_state,
                ..
            } => Self::render_page(self.render_result(
                title.clone(),
                tree_state.clone(),
                editor_state.clone(),
                cx,
            )),
        }
    }

    /// The shared content inset every non-dashboard page sits in.
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
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_match(id, window, cx)
                            }))
                    })),
            )
    }

    fn render_failed(&self, message: SharedString, cx: &Context<Self>) -> impl IntoElement {
        v_flex().size_full().items_center().justify_center().child(
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
        editor_state: Option<Entity<EditorState>>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let copied = self.copied;
        let success = cx.theme().success;
        let copy_button = Button::new("copy-schema")
            .ghost()
            .small()
            .icon(if copied {
                IconName::Check
            } else {
                IconName::Copy
            })
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
                                .size(px(360.))
                                .size_range(px(200.)..px(560.))
                                .child(self.render_structure_tree(tree_state, cx)),
                        )
                        .child(
                            resizable_panel().child(div().size_full().pl_3().map(|pane| {
                                match &editor_state {
                                    Some(state) => pane.child(Editor::new(state).h(relative(1.))),
                                    // The editor is still initialising —
                                    // skeleton lines hold its place.
                                    None => pane.child(v_flex().gap_2().pt_2().children(
                                        (0..14).map(|ix| {
                                            let width = relative(match ix % 4 {
                                                0 => 0.55,
                                                1 => 0.85,
                                                2 => 0.7,
                                                _ => 0.4,
                                            });
                                            Skeleton::new().h_3().w(width)
                                        }),
                                    )),
                                }
                            })),
                        ),
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
            .border_r_1()
            .border_color(cx.theme().border)
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
                        .child(div().text_sm().truncate().child(name))
                        .when_some(counts, |row, counts| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
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
        v_flex()
            .size_full()
            .text_color(cx.theme().foreground)
            .child(self.render_title_bar(cx))
            .child(
                // `h_flex` centres its children; the shell row must stretch
                // so the sidebar and the work area both own the full height.
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(self.render_content(cx)),
            )
            .child(self.render_status_bar(cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
