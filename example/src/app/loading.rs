//! Detail placeholders use Ely's theme-aware, reduced-motion skeletons.
use super::MetabookApp;
use ely_gpui_component::motion::{
    Skeleton, SkeletonAvatar, SkeletonCard, SkeletonTable, SkeletonText,
};
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use gpui::{
    div, rems, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled,
};
use gpui_component::{h_flex, v_flex};

impl MetabookApp {
    pub(super) fn render_detail_loading(
        &self,
        message: SharedString,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .id("book-detail-loading")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .pl_12()
                    .pr_8()
                    .py_8()
                    .gap_8()
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(SkeletonAvatar::new("detail-loading-avatar"))
                            .child(
                                div()
                                    .w(rems(24.))
                                    .child(SkeletonText::new("detail-loading-title", 2)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(cx.theme().text_size(TextSize::Sm))
                            .text_color(cx.theme().colors.fg_muted)
                            .child(message),
                    )
                    .child(
                        div()
                            .w(rems(28.))
                            .child(SkeletonText::new("detail-loading-heading", 1)),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
                            .child(
                                div().flex_1().min_w_0().child(
                                    Skeleton::new("detail-loading-graph").w_full().h(rems(28.)),
                                ),
                            )
                            .child(
                                div()
                                    .w(rems(19.))
                                    .flex_none()
                                    .child(SkeletonCard::new("detail-loading-node")),
                            ),
                    )
                    .child(SkeletonTable::new("detail-loading-fields", 5, 2))
                    .child(SkeletonText::new("detail-loading-structure", 3)),
            )
            .into_any_element()
    }
}
