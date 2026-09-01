//! Form state and value methods for `MetabookApp`.

use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

use super::styles::{DETAIL_OPTIONS, DETAIL_VALUES, TOKENIZER_OPTIONS};
use super::{FormHandles, MetabookApp, Phase};

impl MetabookApp {
    pub(super) fn on_input_event(
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
            tokenizer: self.tokenizer.clone(),
            detail: self.detail.clone(),
            flags: self.flags.clone(),
        }
    }

    pub(super) fn set_flags(&mut self, cx: &mut Context<Self>) {
        let processing = self.is_processing();
        let epub_name = self.epub_path.as_ref().map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
                .into()
        });
        self.flags.update(cx, |flags, _| {
            flags.processing = processing;
            flags.epub_name = epub_name;
        });
    }

    pub(super) fn is_processing(&self) -> bool {
        matches!(self.phase, Phase::Processing { .. })
    }

    /// The API `detail` value for the current select choice.
    pub(super) fn detail_value(&self, cx: &Context<Self>) -> String {
        self.detail
            .read(cx)
            .selected_value()
            .and_then(|label| {
                DETAIL_OPTIONS
                    .iter()
                    .position(|option| option == label)
                    .map(|index| DETAIL_VALUES[index])
            })
            .unwrap_or("paragraph")
            .to_string()
    }

    /// The API `tokenizer` value; empty means "don't count tokens".
    pub(super) fn tokenizer_value(&self, cx: &Context<Self>) -> String {
        self.tokenizer
            .read(cx)
            .selected_value()
            .filter(|label| **label != TOKENIZER_OPTIONS[0])
            .map(|label| label.to_string())
            .unwrap_or_default()
    }
}
