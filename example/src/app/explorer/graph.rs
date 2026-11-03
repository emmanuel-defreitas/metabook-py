//! Ely's graph provides hover and dragging; named controls navigate its data.
use super::graph_data::{self, Focus};
use super::ResultExplorer;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::charts::NetworkGraph;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Radius, TextSize};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, rems, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};
use gpui_component::{h_flex, v_flex, StyledExt as _};

impl ResultExplorer {
    pub(super) fn render_book_graph(&self, cx: &Context<Self>) -> AnyElement {
        let value = &self.value;
        let graph_focus = &self.navigation.graph_focus;
        let view = graph_data::view(value, &self.tree, graph_focus, self.navigation.graph_page);
        let graph_id = format!(
            "{}-{}-{}-{}",
            self.request_ix,
            value["record_id"].as_str().unwrap_or("book"),
            graph_focus.key(),
            view.page
        );
        let mut graph = NetworkGraph::new(gpui::ElementId::Name(graph_id.into()))
            .w_full()
            .h(rems(28.));
        for node in &view.nodes {
            graph = graph.node(graph_data::graph_label(&node.label), node.group);
        }
        for (a, b) in &view.edges {
            graph = graph.edge(*a, *b);
        }

        let controls = h_flex()
            .gap_2()
            .items_center()
            .child(
                Button::new("graph-book", "Book")
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.navigate_graph(Focus::Book, window, cx)
                    })),
            )
            .child(
                Button::new("graph-metadata", "Metadata")
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.navigate_graph(Focus::Json("/book".into()), window, cx)
                    })),
            )
            .child(
                Button::new("graph-structure", "Structure")
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.navigate_graph(Focus::Structure(None), window, cx)
                    })),
            )
            .child(
                Button::new("graph-up", "Up one level")
                    .variant(ButtonVariant::Outline)
                    .size(ControlSize::Sm)
                    .disabled(*graph_focus == Focus::Book)
                    .on_click(cx.listener(|this, _, window, cx| {
                        let parent = this.navigation.graph_focus.parent();
                        cx.stop_propagation();
                        this.navigate_graph(parent, window, cx);
                    })),
            );

        let previous_page = view.page.saturating_sub(1);
        let next_page = (view.page + 1).min(view.pages - 1);
        let selected = &view.nodes[0];
        let list = v_flex()
            .gap_2()
            .children(view.nodes.iter().enumerate().skip(1).map(|(ix, node)| {
                let target = node.target.clone();
                v_flex()
                    .gap_1()
                    .child(
                        Button::new(("graph-node", ix), node.label.clone())
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .full_width()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.navigate_graph(target.clone(), window, cx)
                            })),
                    )
                    .when(!node.detail.is_empty(), |row| {
                        row.child(
                            div()
                                .px_3()
                                .text_color(cx.theme().colors.fg_muted)
                                .text_size(cx.theme().text_size(TextSize::Xs))
                                .child(node.detail.clone()),
                        )
                    })
            }));
        let fields = graph_data::inspected(value, graph_focus);
        let fields = match fields {
            Some(serde_json::Value::Object(fields)) => fields
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(key, v)| {
                    let text = match v {
                        serde_json::Value::Array(items) => format!("{} items", items.len()),
                        serde_json::Value::Object(_) => "Open relationship for details".into(),
                        serde_json::Value::String(text) => text.clone(),
                        _ => v.to_string(),
                    };
                    h_flex()
                        .gap_4()
                        .child(
                            div()
                                .w(rems(14.))
                                .flex_none()
                                .text_color(cx.theme().colors.fg_muted)
                                .child(key.clone()),
                        )
                        .child(div().flex_1().min_w_0().child(text))
                        .into_any_element()
                })
                .collect::<Vec<_>>(),
            Some(serde_json::Value::Array(items)) => vec![div()
                .child(format!(
                    "{} related items · choose a node to inspect",
                    items.len()
                ))
                .into_any_element()],
            Some(v) => vec![div()
                .child(
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                )
                .into_any_element()],
            None => vec![],
        };
        v_flex().gap_4()
            .child(h_flex().flex_wrap().justify_between().gap_4()
                .child(div().font_semibold().child("Metadata & structure graph"))
                .child(controls))
            .child(div().text_color(cx.theme().colors.fg_muted).text_size(cx.theme().text_size(TextSize::Sm))
                .child("Drag nodes to arrange them. Hover to see relationships. Choose a named node to explore its metadata or structure."))
            .child(h_flex().flex_wrap().gap_4().items_start()
                .child(div().flex_1().flex_basis(rems(30.)).min_w(rems(20.)).overflow_hidden().border_1().border_color(cx.theme().colors.border)
                    .rounded(cx.theme().radius(Radius::Lg)).bg(cx.theme().colors.surface).child(graph))
                .child(v_flex().w(rems(19.)).flex_none().gap_3()
                    .child(div().font_semibold().child(selected.label.clone()))
                    .child(div().id("graph-node-list").h(rems(23.)).overflow_y_scroll().child(list))
                    .child(div().text_size(cx.theme().text_size(TextSize::Sm)).text_color(cx.theme().colors.fg_muted)
                        .child(format!("{} children · page {} of {}",view.total,view.page+1,view.pages)))
                    .child(h_flex().gap_2()
                        .child(Button::new("graph-previous", "Previous").variant(ButtonVariant::Outline).size(ControlSize::Sm)
                            .disabled(view.page == 0).on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); this.change_graph_page(previous_page,cx); })))
                        .child(Button::new("graph-next", "Next").variant(ButtonVariant::Outline).size(ControlSize::Sm)
                            .disabled(view.page+1 == view.pages).on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); this.change_graph_page(next_page,cx); }))))))
            .child(v_flex().gap_2().p_4().border_1().border_color(cx.theme().colors.border)
                .rounded(cx.theme().radius(Radius::Lg))
                .child(div().font_semibold().child(format!("Inspect · {}",selected.label)))
                .children(fields))
            .into_any_element()
    }
}
