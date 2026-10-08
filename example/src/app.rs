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

mod components;
mod detail;
mod explorer;
mod home;
mod loading;
mod methods;
mod styles;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ely_gpui_component::forms::TextInput;
use ely_gpui_component::layout::AppShell;
use ely_gpui_component::primitives::FocusScope;
use ely_gpui_component::shell::TitleBar;
use ely_gpui_component::theme::{ActiveTheme as _, Mode, Radius, TextSize, Theme as ElyTheme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, AnyElement, AppContext as _, Context, Entity, Image, ImageFormat, InteractiveElement as _,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window,
};
use gpui_component::button::Button;
use gpui_component::select::SelectState;
use gpui_component::spinner::Spinner;
use gpui_component::{h_flex, v_flex, Root, Sizable as _};

use crate::api::{self, LibraryBook, SearchOutcome, SearchPage};
use explorer::ResultExplorer;
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
    upload_progress: Option<Arc<api::UploadProgress>>,
    tokenizer: Entity<SelectState<Vec<&'static str>>>,
    detail: Entity<SelectState<Vec<&'static str>>>,
    epub_path: Option<PathBuf>,
    phase: Phase,
    library: Library,
    covers: HashMap<SharedString, Cover>,
    /// Shared, theme-colored portrait icon for recent books without a cover.
    preview_placeholder: (gpui::Hsla, Arc<Image>),
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
            query,
            upload_progress: None,
            tokenizer,
            detail,
            epub_path: None,
            phase: Phase::Idle,
            library: Library::Loading,
            covers: HashMap::new(),
            preview_placeholder: home::book_placeholder(cx),
            request_ix: 0,
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
                let explorer = cx.new(|cx| ResultExplorer::new(analysis, ix, window, cx));
                let subscription = cx.observe(&explorer, |_, _, cx| cx.notify());
                Phase::Done {
                    explorer,
                    _subscription: subscription,
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
}

impl Render for MetabookApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::theme::synchronize(cx);
        if self.preview_placeholder.0 != cx.theme().colors.fg_muted {
            self.preview_placeholder = home::book_placeholder(cx);
        }
        let content = v_flex()
            .size_full()
            .font_family(cx.theme().font_family.clone())
            .text_size(cx.theme().text_size(TextSize::Base))
            .text_color(cx.theme().colors.fg)
            .child(
                AppShell::new()
                    .when(!self.is_home(), |shell| {
                        shell.title_bar(
                            v_flex()
                                .child(self.render_title_bar(cx))
                                .child(self.render_toolbar(cx)),
                        )
                    })
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
