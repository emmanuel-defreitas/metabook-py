//! Main application view.
//!
//! A Sketch-aligned library and metadata workspace. A shared search toolbar
//! and EPUB intake sit beside a content region that swaps
//! to the processing, disambiguation, result, and failure views as a request
//! progresses.
//!
//! State ownership: `MetabookApp` owns the workflow phase, the form states,
//! and the persisted library. Each completed analysis owns a retained result
//! explorer for tree/graph/editor coordination. Async requests carry a
//! request index so a stale response can never overwrite a newer one.

mod catalog;
mod components;
mod detail;
mod explorer;
mod home;
mod loading;
mod methods;
mod search_filters;
mod styles;
mod workspace;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ely_gpui_component::forms::TextInput;
use ely_gpui_component::layout::AppShell;
use ely_gpui_component::primitives::FocusScope;
use ely_gpui_component::theme::{ActiveTheme as _, Mode, TextSize, Theme as ElyTheme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, AnyElement, AppContext as _, Context, Entity, Image, ImageFormat, IntoElement,
    ParentElement, PathPromptOptions, Render, SharedString, Styled, Subscription, Window,
};

use gpui_component::select::SelectState;
use gpui_component::spinner::Spinner;
use gpui_component::{v_flex, Root, Sizable as _};

use crate::api::{self, LibraryBook, SearchOutcome, SearchPage};

use explorer::ResultExplorer;
use search_filters::SearchFilters;
use styles::{DETAIL_OPTIONS, TOKENIZER_DEFAULT_IX, TOKENIZER_OPTIONS};

/// The workflow phase shown in the content region.
enum Phase {
    Idle,
    Processing {
        message: SharedString,
        detail_loading: bool,
        home_view: bool,
    },
    /// Catalog metadata with a submitted-filter snapshot and provider-specific actions.
    Matches {
        page: SearchPage,
    },
    Done {
        explorer: Entity<ResultExplorer>,
        // Graph and copy feedback are composed outside the explorer's own
        // Render region, so its notifications must also redraw the shell.
        _subscription: Subscription,
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
    query: Entity<TextInput>,
    filters: SearchFilters,
    upload_progress: Option<Arc<api::UploadProgress>>,
    tokenizer: Entity<SelectState<Vec<&'static str>>>,
    detail: Entity<SelectState<Vec<&'static str>>>,
    epub_path: Option<PathBuf>,
    phase: Phase,
    workspace: workspace::Workspace,
    filters_open: bool,
    library: Library,
    covers: HashMap<SharedString, Cover>,
    /// Incremented per request; responses for an older index are discarded.
    request_ix: usize,
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
            TextInput::new(window, cx)
                .placeholder("Catalog keywords, ISBN, or Gutenberg ID · Enter to search")
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
            query,
            filters: SearchFilters::default(),
            upload_progress: None,
            tokenizer,
            detail,
            epub_path: None,
            phase: Phase::Idle,
            workspace: workspace::Workspace::default(),
            filters_open: false,
            library: Library::Loading,
            covers: HashMap::new(),
            request_ix: 0,
            _subscriptions: subscriptions,
        };
        app.refresh_library(cx);
        app
    }

    // ── Commands ───────────────────────────────────────────────────────────────

    fn select_match(&mut self, gutenberg_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_processing() {
            return;
        }
        let tab_id: SharedString = format!("gutenberg-{gutenberg_id}").into();
        if self.reopen_workspace_tab(&tab_id, cx) { return; }
        self.workspace.next = Some(tab_id);
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
        let gutenberg_id = match &self.library {
            Library::Ready(books) => books.iter().find(|book| book.id == id)
                .and_then(|book| book.record["gutenberg_id"].as_u64()),
            _ => None,
        };
        let tab_id = workspace::book_tab_id(&id, gutenberg_id);
        if self.reopen_workspace_tab(&tab_id, cx) { return; }
        self.workspace.next = Some(tab_id);
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
        self.open_request_tab(cx);
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
        let tab_id = self.workspace.pending.clone().expect("request tab");
        let previous_tab = self.workspace.active.clone();
        self.select_workspace_tab(&tab_id, cx);
        self.phase = match result {
            Ok(SearchOutcome::Analysis(analysis)) => {
                self.workspace.title = analysis.title.clone().into();
                let explorer = cx.new(|cx| ResultExplorer::new(analysis, ix, window, cx));
                let subscription = cx.observe(&explorer, |_, _, cx| cx.notify());
                Phase::Done {
                    explorer,
                    _subscription: subscription,
                }
            }
            Ok(SearchOutcome::Matches(page)) => {
                self.workspace.title = format!("Search results ({})", page.count).into();
                Phase::Matches { page }
            },
            Err(message) => {
                self.workspace.title = "Request failed".into();
                Phase::Failed { message: message.into() }
            },
        };
        // A finished analysis is persisted by the API, so the library list
        // has a new book to show.
        if matches!(self.phase, Phase::Done { .. }) {
            self.refresh_library(cx);
        }
        self.workspace.pending = None;
        self.select_workspace_tab(&previous_tab, cx);
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
            self.open_request_tab(cx);
            self.workspace.pending = None;
            self.workspace.title = "Invalid EPUB".into();
            self.phase = Phase::Failed {
                message: format!("“{}” isn't an .epub file.", path.display()).into(),
            };
        }
        cx.notify();
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
        self.select_workspace_tab(&"home".into(), cx);
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

    fn render_title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_home_chrome(window, cx)
    }

    /// The work area. Idle shows the library (its own scroll owner, so the
    /// scrollbar sits at the panel edge); every other phase is a page inside
    /// the shared content inset.
    fn render_content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match &self.phase {
            Phase::Idle => self.render_home(window, cx),
            Phase::Processing {
                home_view: true, ..
            } => self.render_home(window, cx),
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
                            .child(self.render_title_bar(window, cx))
                            .child(self.render_toolbar(cx)),
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
                            .child(self.render_content(window, cx)),
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
