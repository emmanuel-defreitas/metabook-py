//! Pure helpers for adapting API tree data to GPUI tree items.

use std::collections::HashSet;

use gpui::SharedString;
use gpui_component::tree::TreeItem;

use crate::api::TreeNode;

/// Joins a node's name and counts inside the single label a `TreeItem` carries.
/// A control character cannot appear in book text, so it cannot accidentally
/// split a chapter heading.
pub(super) const META_SEPARATOR: char = '\u{1f}';

/// Build `TreeItem`s for only the visible portion of the source tree.
pub(super) fn materialize_items(
    nodes: &[TreeNode],
    expanded: &HashSet<SharedString>,
) -> Vec<TreeItem> {
    nodes
        .iter()
        .map(|node| materialize_item(node, expanded))
        .collect()
}

fn materialize_item(node: &TreeNode, expanded: &HashSet<SharedString>) -> TreeItem {
    let id = SharedString::from(node.id.clone());
    let is_expanded = expanded.contains(&id);
    let label = if node.meta.is_empty() {
        node.label.clone()
    } else {
        format!("{}{META_SEPARATOR}{}", node.label, node.meta)
    };
    let item = TreeItem::new(id, SharedString::from(label)).expanded(is_expanded);

    if node.children.is_empty() {
        item
    } else if is_expanded {
        item.children(
            node.children
                .iter()
                .map(|child| materialize_item(child, expanded))
                .collect::<Vec<_>>(),
        )
    } else {
        // A hidden placeholder preserves the collapsed folder's chevron.
        item.child(TreeItem::new(
            SharedString::from(format!("{}.placeholder", node.id)),
            SharedString::default(),
        ))
    }
}
