//! Coherent navigation shared by the graph, tree, and editor.

use std::collections::HashSet;

use gpui::SharedString;

use super::graph_data::Focus;

#[derive(Default)]
pub(super) struct Navigation {
    pub(super) selected_node: Option<String>,
    pub(super) graph_focus: Focus,
    pub(super) graph_page: usize,
    pub(super) expanded: HashSet<SharedString>,
}

impl Navigation {
    /// Repeated tree notifications must not reset graph pagination.
    pub(super) fn select_structure(&mut self, selected: Option<String>) -> bool {
        if self.selected_node == selected {
            return false;
        }
        self.graph_focus = Focus::Structure(selected.clone());
        self.selected_node = selected;
        self.graph_page = 0;
        true
    }

    pub(super) fn navigate(&mut self, focus: Focus) {
        if let Focus::Structure(Some(id)) = &focus {
            let mut ancestor = id.as_str();
            while let Some((parent, _)) = ancestor.rsplit_once('.') {
                self.expanded.insert(parent.to_string().into());
                ancestor = parent;
            }
            self.selected_node = Some(id.clone());
        }
        // Metadata navigation changes graph focus only; keep the structural
        // selection available in the adjacent fields/editor pane.
        self.graph_focus = focus;
        self.graph_page = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_result_starts_at_book_without_tree_selection() {
        let state = Navigation::default();
        assert_eq!(state.graph_focus, Focus::Book);
        assert!(state.selected_node.is_none());
        assert!(state.expanded.is_empty());
        assert_eq!(state.graph_page, 0);
    }

    #[test]
    fn tree_selection_focuses_graph_and_resets_pagination() {
        let mut state = Navigation {
            graph_page: 3,
            ..Default::default()
        };
        assert!(state.select_structure(Some("n0.1".into())));
        assert_eq!(state.graph_focus, Focus::Structure(Some("n0.1".into())));
        assert_eq!(state.graph_page, 0);
    }

    #[test]
    fn repeated_tree_notification_keeps_graph_page_and_metadata_focus() {
        let mut state = Navigation::default();
        state.select_structure(Some("n0".into()));
        state.navigate(Focus::Json("/book".into()));
        state.graph_page = 2;
        assert!(!state.select_structure(Some("n0".into())));
        assert_eq!(state.graph_page, 2);
        assert_eq!(state.graph_focus, Focus::Json("/book".into()));
    }

    #[test]
    fn graph_navigation_expands_only_target_ancestors_and_selects_node() {
        let mut state = Navigation::default();
        state.navigate(Focus::Structure(Some("n0.2.3".into())));
        assert_eq!(state.selected_node.as_deref(), Some("n0.2.3"));
        assert_eq!(
            state.expanded,
            HashSet::from([SharedString::from("n0"), SharedString::from("n0.2")])
        );
    }

    #[test]
    fn graph_metadata_navigation_preserves_tree_selection_and_expansion() {
        let mut state = Navigation::default();
        state.navigate(Focus::Structure(Some("n0.1".into())));
        state.navigate(Focus::Json("/book/authors".into()));
        assert_eq!(state.selected_node.as_deref(), Some("n0.1"));
        assert!(state.expanded.contains("n0"));
        assert_eq!(state.graph_focus, Focus::Json("/book/authors".into()));
    }

    #[test]
    fn clearing_tree_selection_focuses_structure_root() {
        let mut state = Navigation::default();
        state.select_structure(Some("n0".into()));
        assert!(state.select_structure(None));
        assert_eq!(state.graph_focus, Focus::Structure(None));
    }
}
