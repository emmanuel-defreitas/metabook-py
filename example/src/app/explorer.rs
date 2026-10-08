//! One retained owner for a result's tree, graph, and JSON editor.
//!
//! The shell composes the graph and copy action around its metadata sections,
//! while this entity renders the structural panes. Subscriptions and deferred
//! work belong to this result, never to whichever result the shell opens next.

mod graph;
mod graph_data;
mod navigation;
mod tree;
mod view;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, AppContext as _, ClipboardItem, Context, Entity, HighlightStyle, IntoElement,
    Render, SharedString, Subscription, Task, Window,
};
use gpui_component::input::{EditorState, Position, TextDecoration, TextDecorationCollection};
use gpui_component::tree::{TreeEvent, TreeState};
use gpui_component::ActiveTheme as _;
use serde_json::Value;

use crate::api::{Analysis, NodeSpan, TreeNode};
use graph_data::Focus;
use navigation::Navigation;
use tree::materialize_items;

/// Separate regions let the shell retain the existing metadata layout without
/// reaching into navigation or editor state.
pub(super) struct Presentation {
    pub(super) graph: AnyElement,
    pub(super) copy: ely_gpui_component::buttons::Button,
}

pub(super) struct ResultExplorer {
    title: SharedString,
    value: Rc<Value>,
    schema_json: SharedString,
    ranges: HashMap<String, NodeSpan>,
    tree: Vec<TreeNode>,
    navigation: Navigation,
    tree_state: Entity<TreeState>,
    editor_state: Option<Entity<EditorState>>,
    decorations: Option<TextDecorationCollection>,
    full_json: bool,
    copied: bool,
    request_ix: usize,
    expand_gen: u64,
    last_expanded: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
    _editor_task: Task<()>,
    _copy_reset: Option<Task<()>>,
}

impl ResultExplorer {
    pub(super) fn new(
        analysis: Analysis,
        request_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let navigation = Navigation::default();
        let items = materialize_items(&analysis.tree, &navigation.expanded);
        let tree_state = cx.new(|cx| TreeState::new(cx).items(items));
        // Selection is observed; expansion has its own semantic event. Keep
        // both subscriptions alive only for the lifetime of this explorer.
        let subscriptions = vec![
            cx.observe_in(&tree_state, window, Self::on_tree_changed),
            cx.subscribe(&tree_state, |this, _, event: &TreeEvent, cx| {
                this.on_tree_toggle(event, cx);
            }),
        ];
        // Give the result a chance to paint before constructing a potentially
        // multi-megabyte editor rope. The weak target is this exact result.
        let editor_task = cx.spawn_in(window, async move |this, cx| {
            this.update_in(cx, |this, window, cx| this.init_editor(window, cx))
                .ok();
        });

        Self {
            title: analysis.title.into(),
            value: Rc::new(analysis.value),
            schema_json: analysis.schema_json.into(),
            ranges: analysis.ranges,
            tree: analysis.tree,
            navigation,
            tree_state,
            editor_state: None,
            decorations: None,
            full_json: false,
            copied: false,
            request_ix,
            expand_gen: 0,
            last_expanded: None,
            _subscriptions: subscriptions,
            _editor_task: editor_task,
            _copy_reset: None,
        }
    }

    pub(super) fn title(&self) -> &SharedString {
        &self.title
    }

    pub(super) fn value(&self) -> &Rc<Value> {
        &self.value
    }

    fn init_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_state.is_some() {
            return;
        }
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .folding(true)
                .default_value(self.schema_json.clone())
        });
        let decorations = state.update(cx, |state, cx| {
            state.set_readonly(true, cx);
            state.create_decorations_collection(vec![], cx)
        });
        self.editor_state = Some(state);
        self.decorations = Some(decorations);
        // A tree selection may precede editor readiness; apply it now rather
        // than waiting for the user to select a different node.
        self.sync_editor(window, cx);
        cx.notify();
    }

    fn on_tree_toggle(&mut self, event: &TreeEvent, cx: &mut Context<Self>) {
        let changed = match event {
            TreeEvent::Expanded(id) => {
                self.last_expanded = Some(id.clone());
                self.expand_gen += 1;
                self.navigation.expanded.insert(id.clone())
            }
            TreeEvent::Collapsed(id) => self.navigation.expanded.remove(id),
        };
        if changed {
            let selected = self
                .tree_state
                .read(cx)
                .selected_entry()
                .map(|entry| entry.item().id.clone());
            self.rebuild_tree(selected.as_ref(), cx);
            cx.notify();
        }
    }

    fn collapse_all(&mut self, cx: &mut Context<Self>) {
        if self.navigation.expanded.is_empty() {
            return;
        }
        self.navigation.expanded.clear();
        self.last_expanded = None;
        self.rebuild_tree(None, cx);
        cx.notify();
    }

    fn rebuild_tree(&mut self, selected: Option<&SharedString>, cx: &mut Context<Self>) {
        let items = materialize_items(&self.tree, &self.navigation.expanded);
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
            // Visible row positions change when siblings expand; restore the
            // domain ID, not a positional index into the old materialization.
            let ix = selected.and_then(|id| state.index_of(id));
            state.set_selected_index(ix, cx);
        });
    }

    fn on_tree_changed(
        &mut self,
        state: Entity<TreeState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = state
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.to_string());
        if self.navigation.select_structure(selected) {
            self.sync_editor(window, cx);
            cx.notify();
        }
    }

    fn navigate_graph(&mut self, focus: Focus, window: &mut Window, cx: &mut Context<Self>) {
        let selected = match &focus {
            Focus::Structure(Some(id)) => Some(SharedString::from(id.clone())),
            _ => None,
        };
        self.navigation.navigate(focus);
        if let Some(id) = selected {
            self.rebuild_tree(Some(&id), cx);
            self.sync_editor(window, cx);
        }
        cx.notify();
    }

    fn change_graph_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.navigation.graph_page = page;
        cx.notify();
    }

    fn sync_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(editor), Some(decorations)) = (&self.editor_state, &self.decorations) else {
            return;
        };
        let span = self
            .navigation
            .selected_node
            .as_ref()
            .and_then(|id| self.ranges.get(id));
        if let Some(span) = span {
            editor.update(cx, |state, cx| {
                let position = Position::new(span.line as u32, 0);
                state.unfold_at(position, cx);
                state.set_cursor_position(position, window, cx);
            });
            // The retained editor uses the theme bridge's selection token.
            decorations.set(
                vec![TextDecoration::new(
                    span.bytes.clone(),
                    HighlightStyle {
                        background_color: Some(cx.theme().selection),
                        ..Default::default()
                    },
                )],
                cx,
            );
        } else {
            decorations.set(Vec::new(), cx);
        }
    }

    fn copy_schema(&mut self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.schema_json.to_string()));
        self.copied = true;
        cx.notify();
        // Repeated copying restarts this result's feedback timer; dropping the
        // result cancels it rather than mutating a subsequently opened book.
        self._copy_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1400))
                .await;
            this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            })
            .ok();
        }));
    }
}

impl Render for ResultExplorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_structure_explorer(cx)
    }
}
