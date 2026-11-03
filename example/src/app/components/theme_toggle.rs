//! The shared window-chrome theme action, sized by Ely's large control token.

use ely_gpui_component::{
    buttons::IconButton,
    primitives::IconName,
    theme::{ActiveTheme as _, ControlSize},
};
use gpui::{AnyElement, Context, IntoElement};

use super::super::MetabookApp;

impl MetabookApp {
    pub(in crate::app) fn render_theme_toggle(&self, cx: &Context<Self>) -> AnyElement {
        let dark = cx.theme().is_dark();
        IconButton::new(
            "titlebar-dark-mode-toggle",
            if dark { IconName::Sun } else { IconName::Moon },
        )
        .size(ControlSize::Lg)
        .tooltip(if dark {
            "Switch to light mode"
        } else {
            "Switch to dark mode"
        })
        .on_click(cx.listener(|this, _, window, cx| {
            cx.stop_propagation();
            this.toggle_theme(window, cx);
        }))
        .into_any_element()
    }
}
