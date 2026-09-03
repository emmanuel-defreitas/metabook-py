//! Interaction tests for the result split: dragging the divider between the
//! structure tree and the JSON editor must resize the panels, not fall
//! through to the editor's text selection.

use gpui::{
    div, point, prelude::FluentBuilder as _, px, relative, AppContext as _, Context, Entity,
    IntoElement, Modifiers, MouseButton, ParentElement as _, Pixels, Render, Styled as _,
    TestAppContext, VisualTestContext, Window,
};
use gpui_component::input::{Editor, EditorState};
use gpui_component::list::ListItem;
use gpui_component::resizable::{h_resizable, resizable_panel, ResizableState};
use gpui_component::tree::{tree, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};

/// Mirrors `MetabookApp::render_result`: a 360px tree pane with its own
/// right border, then the JSON editor pane, split by `h_resizable`.
struct SplitHarness {
    resizable: Entity<ResizableState>,
    editor: Option<Entity<EditorState>>,
    tree_state: Option<Entity<TreeState>>,
}

impl Render for SplitHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The same nesting as MetabookApp::render: page padding, then the
        // split, tree pane with its border, editor pane with its inset.
        let tree_pane = v_flex()
            .size_full()
            .pr_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(div().flex_1().min_h_0().map(|slot| match &self.tree_state {
                Some(state) => slot.child(tree(state, |ix, entry, selected, _, _| {
                    ListItem::new(ix).selected(selected).child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::File).small())
                            .child(div().text_sm().truncate().child(entry.item().label.clone())),
                    )
                })),
                None => slot.child(div().size_full()),
            }));
        v_flex()
            .size(px(1200.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                v_flex().flex_1().min_h_0().gap_3().p_4().pt_2().child(
                    div().flex_1().min_h_0().child(
                        h_resizable("result-split")
                            .with_state(&self.resizable)
                            .child(
                                resizable_panel()
                                    .size(px(360.))
                                    .size_range(px(200.)..px(560.))
                                    .child(tree_pane),
                            )
                            .child(resizable_panel().child({
                                let pane = div().size_full().pl_3();
                                match &self.editor {
                                    Some(state) => pane.child(Editor::new(state).h(relative(1.))),
                                    None => pane.child(div().size_full()),
                                }
                            })),
                    ),
                ),
            )
    }
}

fn harness(
    cx: &mut TestAppContext,
    with_editor: bool,
) -> (
    &mut VisualTestContext,
    Entity<ResizableState>,
    Option<Entity<EditorState>>,
) {
    let mut split: Option<Entity<SplitHarness>> = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        gpui_component::init(cx);
        let resizable = cx.new(|_| ResizableState::default());
        let editor = with_editor.then(|| {
            // Long enough to scroll, like a real schema payload.
            let mut json = String::from("{\n  \"schema\": \"standard_book\",\n  \"nodes\": [\n");
            for ix in 0..400 {
                json.push_str(&format!(
                    "    {{ \"index\": {ix}, \"sentence_count\": 2, \"word_count\": 24 }},\n"
                ));
            }
            json.push_str("  ]\n}\n");
            let state = cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("json")
                    .line_number(true)
                    .folding(true)
                    .default_value(json)
            });
            state.update(cx, |state, cx| {
                state.set_readonly(true, cx);
                state.create_decorations_collection(vec![], cx);
            });
            state
        });
        let items = (0..60)
            .map(|ix| TreeItem::new(format!("n{ix}"), format!("Paragraph {ix}")))
            .collect::<Vec<_>>();
        let tree_state = Some(cx.new(|cx| TreeState::new(cx).items(items)));
        let view = cx.new(|_| SplitHarness {
            resizable,
            editor,
            tree_state,
        });
        split = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let (resizable, editor) = split
        .unwrap()
        .read_with(cx, |view, _| (view.resizable.clone(), view.editor.clone()));
    (cx, resizable, editor)
}

fn drag_divider(cx: &mut VisualTestContext, from_x: Pixels, to_x: Pixels) {
    cx.simulate_mouse_down(
        point(from_x, px(300.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(from_x + px(6.), px(300.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(to_x, px(300.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(to_x, px(300.)),
        MouseButton::Left,
        Modifiers::default(),
    );
}

/// The split sits inside the page's `p_4` inset, so panel 1's left edge is
/// 16px into the window and the divider is at 16 + sizes[0].
const PAGE_INSET: f32 = 16.;

#[track_caller]
fn assert_size(actual: Pixels, expected: f32, message: &str) {
    assert!(
        (f32::from(actual) - expected).abs() < 0.5,
        "{message}: panel size is {actual}, expected ~{expected}px"
    );
}

#[gpui::test]
fn divider_drag_resizes_plain_panes(cx: &mut TestAppContext) {
    let (cx, resizable, _) = harness(cx, false);
    let boundary = resizable.read_with(cx, |state, _| state.sizes()[0]) + px(PAGE_INSET);
    drag_divider(cx, boundary - px(2.), boundary + px(40.));
    let size = resizable.read_with(cx, |state, _| state.sizes()[0]);
    assert_size(size, 400., "plain panes: drag should resize");
}

#[gpui::test]
fn divider_drag_resizes_next_to_the_editor(cx: &mut TestAppContext) {
    let (cx, resizable, editor) = harness(cx, true);
    let editor = editor.unwrap();
    let boundary = resizable.read_with(cx, |state, _| state.sizes()[0]) + px(PAGE_INSET);

    // Real usage order: work in the editor first (focus it, leave the mouse
    // hovering it), then go for the divider.
    cx.simulate_mouse_down(
        point(boundary + px(120.), px(300.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(boundary + px(120.), px(300.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));

    drag_divider(cx, boundary - px(2.), boundary + px(40.));
    let size = resizable.read_with(cx, |state, _| state.sizes()[0]);
    assert_size(
        size,
        400.,
        "editor pane: drag should resize, not select text",
    );
    // And the drag must not have started a text selection in the editor.
    let selection_empty = editor.read_with(cx, |state, _| state.selected_range().is_empty());
    assert!(
        selection_empty,
        "the drag leaked into the editor as a text selection"
    );
}

/// The handle's hit area spans HANDLE_PADDING (4px) each side of the panel
/// boundary. Every x offset inside that zone must start a resize — and none
/// of them may leak into the editor as a text selection. Each probe
/// approaches with hover moves first, like a real pointer would.
#[gpui::test]
fn divider_hit_area_covers_the_boundary(cx: &mut TestAppContext) {
    let (cx, resizable, editor) = harness(cx, true);
    let editor = editor.unwrap();
    let mut failures = String::new();
    // Strictly inside the ±4px zone: its exact edges can land on device-pixel
    // rounding of the hitbox bounds.
    for tenths in (-35i32..=35).step_by(14) {
        let offset = tenths as f32 / 10.;
        let start = resizable.read_with(cx, |state, _| state.sizes()[0]);
        let boundary = start + px(PAGE_INSET);
        let from = boundary + px(offset);
        // Approach: hover moves toward the divider, then press-drag-release.
        cx.simulate_mouse_move(point(from + px(80.), px(300.)), None, Modifiers::default());
        cx.simulate_mouse_move(point(from + px(20.), px(300.)), None, Modifiers::default());
        cx.simulate_mouse_move(point(from, px(300.)), None, Modifiers::default());
        cx.simulate_mouse_down(
            point(from, px(300.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(from + px(5.5), px(300.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(from + px(30.5), px(302.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(from + px(30.5), px(302.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let end = resizable.read_with(cx, |state, _| state.sizes()[0]);
        let selected = editor.read_with(cx, |state, _| !state.selected_range().is_empty());
        if (f32::from(end) - f32::from(start)).abs() < 1. || selected {
            failures.push_str(&format!(
                "offset {offset:+.1}: resized {} -> {}, editor selected: {selected}\n",
                f32::from(start),
                f32::from(end),
            ));
        }
        // Reset for the next probe.
        cx.update(|window, cx| {
            resizable.update(cx, |state, cx| state.resize_panel(0, px(360.), window, cx));
            editor.update(cx, |state, cx| state.unselect(window, cx));
            window.draw(cx).clear(cx);
        });
    }
    assert!(
        failures.is_empty(),
        "dead spots in the divider hit area:\n{failures}"
    );
}
