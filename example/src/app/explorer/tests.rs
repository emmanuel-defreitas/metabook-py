//! Entity tests deliberately render no Ely controls: TestAppContext has no
//! Ely assets. Exercise retained behavior through the same navigation paths.

use gpui::{div, TestAppContext, VisualTestContext};
use serde_json::json;

use super::*;

struct TestRoot {
    explorer: Option<Entity<ResultExplorer>>,
}

impl Render for TestRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn analysis(title: &str) -> Analysis {
    let value = json!({
        "book": {"title": title},
        "structure": {
            "schema": "standard_book",
            "nodes": [
                {"level": "chapter", "index": 1, "paragraph_count": 1,
                 "paragraphs": [{"index": 1, "word_count": 4}]},
                {"level": "chapter", "index": 2, "paragraph_count": 0}
            ]
        }
    });
    let tree = crate::api::build_tree(&value["structure"]);
    let schema_json = serde_json::to_string_pretty(&value).unwrap();
    let words = schema_json.find("\"word_count\"").unwrap();
    let start = schema_json[..words].rfind('{').unwrap();
    let end = words + schema_json[words..].find('}').unwrap() + 1;
    let ranges = HashMap::from([(
        "n0.0".into(),
        NodeSpan {
            line: schema_json[..start].matches('\n').count(),
            bytes: start..end,
        },
    )]);
    Analysis {
        title: title.into(),
        value,
        schema_json,
        tree,
        ranges,
    }
}

fn harness(cx: &mut TestAppContext) -> (Entity<TestRoot>, &mut VisualTestContext) {
    cx.add_window_view(|window, cx| {
        gpui_component::init(cx);
        let explorer = cx.new(|cx| ResultExplorer::new(analysis("First book"), 1, window, cx));
        TestRoot {
            explorer: Some(explorer),
        }
    })
}

fn explorer(root: &Entity<TestRoot>, cx: &VisualTestContext) -> Entity<ResultExplorer> {
    root.read_with(cx, |root, _| root.explorer.clone().unwrap())
}

#[gpui::test]
fn graph_navigation_selects_lazy_tree_and_moves_editor_cursor(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let explorer = explorer(&root, cx);
    cx.run_until_parked();
    cx.update(|window, cx| {
        explorer.update(cx, |this, cx| {
            this.navigate_graph(Focus::Structure(Some("n0.0".into())), window, cx);
        });
    });
    explorer.read_with(cx, |this, cx| {
        let selected = this.tree_state.read(cx).selected_entry().unwrap();
        assert_eq!(selected.item().id.as_ref(), "n0.0");
        assert_eq!(
            this.navigation.graph_focus,
            Focus::Structure(Some("n0.0".into()))
        );
        let editor = this.editor_state.as_ref().unwrap().read(cx);
        assert!(!editor.is_editable());
        assert_eq!(
            editor.cursor_position(),
            Position::new(this.ranges["n0.0"].line as u32, 0)
        );
    });
}

#[gpui::test]
fn editor_initialization_replays_selection_made_while_loading(cx: &mut TestAppContext) {
    // The test dispatcher can poll tasks as soon as window construction
    // returns, so make the early selection inside that construction.
    let (root, cx) = cx.add_window_view(|window, cx| {
        gpui_component::init(cx);
        let explorer = cx.new(|cx| ResultExplorer::new(analysis("Book"), 1, window, cx));
        explorer.update(cx, |this, cx| {
            assert!(this.editor_state.is_none());
            this.navigate_graph(Focus::Structure(Some("n0.0".into())), window, cx);
        });
        TestRoot {
            explorer: Some(explorer),
        }
    });
    let explorer = explorer(&root, cx);
    cx.run_until_parked();
    explorer.read_with(cx, |this, cx| {
        let editor = this.editor_state.as_ref().unwrap().read(cx);
        assert_eq!(
            editor.cursor_position(),
            Position::new(this.ranges["n0.0"].line as u32, 0)
        );
    });
}

#[gpui::test]
fn expanding_an_earlier_sibling_preserves_selected_node_identity(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let explorer = explorer(&root, cx);
    cx.update(|_, cx| {
        explorer.update(cx, |this, cx| {
            this.tree_state.update(cx, |tree, cx| {
                let ix = tree.index_of(&"n1".into());
                tree.set_selected_index(ix, cx);
            });
            this.on_tree_toggle(&TreeEvent::Expanded("n0".into()), cx);
        });
    });
    explorer.read_with(cx, |this, cx| {
        assert_eq!(
            this.tree_state
                .read(cx)
                .selected_entry()
                .unwrap()
                .item()
                .id
                .as_ref(),
            "n1"
        );
    });
}

#[gpui::test]
fn collapse_all_clears_tree_and_graph_selection(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let explorer = explorer(&root, cx);
    cx.update(|window, cx| {
        explorer.update(cx, |this, cx| {
            this.navigate_graph(Focus::Structure(Some("n0.0".into())), window, cx);
            this.collapse_all(cx);
        });
    });
    cx.run_until_parked();
    explorer.read_with(cx, |this, cx| {
        assert!(this.navigation.expanded.is_empty());
        assert!(this.tree_state.read(cx).selected_entry().is_none());
        assert_eq!(this.navigation.graph_focus, Focus::Structure(None));
    });
}

#[gpui::test]
fn replacing_result_resets_feedback_and_drops_old_explorer(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let old = explorer(&root, cx);
    let weak = old.downgrade();
    cx.update(|window, cx| {
        old.update(cx, |this, cx| {
            this.full_json = true;
            this.navigate_graph(Focus::Structure(Some("n0.0".into())), window, cx);
            this.copy_schema(cx);
        });
        let next = cx.new(|cx| ResultExplorer::new(analysis("Second book"), 2, window, cx));
        root.update(cx, |root, _| root.explorer = Some(next));
    });
    drop(old);
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());
    let next = explorer(&root, cx);
    next.read_with(cx, |this, cx| {
        assert_eq!(this.title().as_ref(), "Second book");
        assert!(!this.full_json);
        assert!(!this.copied);
        assert_eq!(this.navigation.graph_focus, Focus::Book);
        assert!(this.tree_state.read(cx).selected_entry().is_none());
        assert_eq!(
            this.editor_state
                .as_ref()
                .unwrap()
                .read(cx)
                .text()
                .to_string(),
            this.schema_json.as_ref()
        );
    });
}

#[gpui::test]
fn repeated_copy_restarts_feedback_timer(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let explorer = explorer(&root, cx);
    cx.update(|_, cx| explorer.update(cx, |this, cx| this.copy_schema(cx)));
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(1000));
    cx.update(|_, cx| explorer.update(cx, |this, cx| this.copy_schema(cx)));
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(explorer.read_with(cx, |this, _| this.copied));
    cx.background_executor
        .advance_clock(Duration::from_millis(900));
    cx.run_until_parked();
    assert!(!explorer.read_with(cx, |this, _| this.copied));
}

#[gpui::test]
fn closing_result_before_editor_initialization_does_not_affect_replacement(
    cx: &mut TestAppContext,
) {
    let mut previous = None;
    let (root, cx) = cx.add_window_view(|window, cx| {
        gpui_component::init(cx);
        let old = cx.new(|cx| ResultExplorer::new(analysis("Old book"), 1, window, cx));
        assert!(old.read(cx).editor_state.is_none());
        previous = Some(old.downgrade());
        drop(old);
        let next = cx.new(|cx| ResultExplorer::new(analysis("New book"), 2, window, cx));
        TestRoot {
            explorer: Some(next),
        }
    });
    cx.run_until_parked();
    assert!(previous.unwrap().upgrade().is_none());
    explorer(&root, cx).read_with(cx, |this, cx| {
        assert_eq!(this.title().as_ref(), "New book");
        assert_eq!(
            this.editor_state
                .as_ref()
                .unwrap()
                .read(cx)
                .text()
                .to_string(),
            this.schema_json.as_ref()
        );
    });
}

#[gpui::test]
fn old_copy_timer_cannot_clear_replacement_feedback(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let old = explorer(&root, cx);
    cx.update(|_, cx| old.update(cx, |this, cx| this.copy_schema(cx)));
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(1000));
    cx.update(|window, cx| {
        let next = cx.new(|cx| ResultExplorer::new(analysis("Next book"), 2, window, cx));
        next.update(cx, |this, cx| this.copy_schema(cx));
        root.update(cx, |root, _| root.explorer = Some(next));
    });
    drop(old);
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(explorer(&root, cx).read_with(cx, |this, _| this.copied));
}

#[gpui::test]
fn retained_tree_does_not_keep_removed_result_alive(cx: &mut TestAppContext) {
    let (root, cx) = harness(cx);
    let explorer = explorer(&root, cx);
    let weak = explorer.downgrade();
    let tree = explorer.read_with(cx, |this, _| this.tree_state.clone());
    cx.update(|_, cx| root.update(cx, |root, _| root.explorer = None));
    drop(explorer);
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());
    // A notification after teardown must not invoke a removed explorer.
    tree.update(cx, |tree, cx| tree.set_selected_index(Some(0), cx));
    cx.run_until_parked();
}
