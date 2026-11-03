//! Retained result explorer presentation and lazy structural tree rendering.

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use super::graph_data;
use super::tree::META_SEPARATOR;
use super::{Presentation, ResultExplorer};
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::motion::SkeletonText;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, radians, relative, AnyElement, Context, ElementId, Entity, IntoElement, ParentElement,
    Styled,
};
use gpui_component::button::{Button as TreeButton, ButtonVariants as _};
use gpui_component::input::Editor;
use gpui_component::list::ListItem;
use gpui_component::tree::{tree, TreeState};
use gpui_component::{h_flex, v_flex, Icon, IconName, Sizable as _, StyledExt as _};
use gpui_motion::{MotionExt as _, Spring, Tween};
use serde_json::Value;

impl ResultExplorer {
    pub(in crate::app) fn presentation(&self, cx: &Context<Self>) -> Presentation {
        let copy = Button::new(
            "copy-schema",
            if self.copied { "Copied" } else { "Copy JSON" },
        )
        .variant(ButtonVariant::Outline)
        .size(ControlSize::Sm)
        .on_click(cx.listener(|this, _, _, cx| this.copy_schema(cx)));
        Presentation {
            graph: self.render_book_graph(cx),
            copy,
        }
    }

    pub(super) fn render_structure_explorer(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .gap(gpui::rems(3.5))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .child(section_heading("Structure explorer", cx))
                    .child(
                        div()
                            .h_80()
                            .child(self.render_structure_tree(self.tree_state.clone(), cx)),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(section_heading(
                                if self.full_json {
                                    "Full structural JSON"
                                } else {
                                    "Selected node · JSON fields"
                                },
                                cx,
                            ))
                            .child(
                                Button::new(
                                    "view-full-json",
                                    if self.full_json {
                                        "Selected node"
                                    } else {
                                        "Full JSON"
                                    },
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ControlSize::Sm)
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.full_json = !this.full_json;
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .h_80()
                            .when(self.full_json, |pane| match self.editor_state.as_ref() {
                                Some(state) => pane.child(Editor::new(state).h(relative(1.))),
                                None => pane.child(SkeletonText::new("loading-json-editor", 12)),
                            })
                            .when(!self.full_json, |pane| {
                                pane.child(self.render_selected_fields(
                                    &self.value,
                                    self.navigation.selected_node.as_deref(),
                                    cx,
                                ))
                            }),
                    )
                    .child(detail_note(
                        "Counts and positions only · no source prose",
                        cx,
                    )),
            )
            .into_any_element()
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
                    TreeButton::new("collapse-all")
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

    fn render_selected_fields(
        &self,
        value: &Rc<Value>,
        selected: Option<&str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let fields = selected.and_then(|id| selected_fields(value, id));
        let Some(fields) = fields else {
            return detail_note(
                "Select a chapter, paragraph, or sentence to inspect its structural fields.",
                cx,
            );
        };
        let last = fields.len().saturating_sub(1);
        v_flex()
            .gap_1()
            .font_family(cx.theme().mono_family.clone())
            .text_size(cx.theme().text_size(TextSize::Xs))
            .child("{")
            .children(fields.into_iter().enumerate().map(|(ix, (key, value))| {
                div()
                    .pl_4()
                    .text_color(cx.theme().colors.link)
                    .child(format!(
                        "\"{key}\": {value}{}",
                        if ix < last { "," } else { "" }
                    ))
            }))
            .child("}")
            .into_any_element()
    }
}

fn section_heading(title: &'static str, cx: &Context<ResultExplorer>) -> AnyElement {
    div()
        .w_full()
        .pb_2()
        .border_b_1()
        .border_color(cx.theme().colors.border)
        .text_size(cx.theme().text_size(TextSize::Base))
        .font_medium()
        .child(title)
        .into_any_element()
}

fn detail_note(note: &'static str, cx: &Context<ResultExplorer>) -> AnyElement {
    div()
        .text_size(cx.theme().text_size(TextSize::Xs))
        .text_color(cx.theme().colors.fg_muted)
        .child(note)
        .into_any_element()
}

fn selected_fields(value: &Value, id: &str) -> Option<Vec<(String, String)>> {
    Some(
        graph_data::structure_value(value, id)?
            .as_object()?
            .iter()
            .filter(|(_, v)| !v.is_array() && !v.is_object())
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_fields_follow_nested_nodes_without_including_children() {
        let value = serde_json::json!({"structure":{"nodes":[{"index":1,"paragraphs":[{"index":2,"word_count":4,"sentences":[{"index":1}]}]}]}});
        let fields = selected_fields(&value, "n0.0").unwrap();
        assert!(fields.contains(&("word_count".into(), "4".into())));
        assert!(!fields.iter().any(|(key, _)| key == "sentences"));
        assert!(selected_fields(&value, "n0.8").is_none());
    }
}
