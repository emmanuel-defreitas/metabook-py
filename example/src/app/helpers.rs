//! Pure helpers: adapting API tree data to GPUI tree items, and the EPUB
//! intake gate shared by the drop zone and the file dialog.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

/// The one EPUB a drop or the file dialog brings, or why it is refused.
///
/// Follows the gate of Ely's `forms::DropZone` (single-file mode, one kind):
/// nothing dropped, several files, a folder, or another extension are each
/// refused with a reason the dashboard can show. The extension match ignores
/// case, so `BOOK.EPUB` is taken.
pub(super) fn epub_from(paths: &[PathBuf]) -> Result<PathBuf, &'static str> {
    match paths {
        [] => Err("Nothing was dropped."),
        [path] if !path.is_file() => Err("Drop a file, not a folder."),
        [path] if !is_epub(path) => Err("Only .epub files can be scanned."),
        [path] => Ok(path.clone()),
        _ => Err("Drop one EPUB at a time."),
    }
}

fn is_epub(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("epub"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A scratch directory unique to one test, with the named files created.
    fn scratch(test: &str, files: &[&str]) -> (PathBuf, Vec<PathBuf>) {
        let dir = std::env::temp_dir().join(format!("metabook-drop-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        let paths = files
            .iter()
            .map(|name| {
                let path = dir.join(name);
                fs::write(&path, b"x").expect("scratch file");
                path
            })
            .collect();
        (dir, paths)
    }

    #[test]
    fn takes_one_epub_in_any_case() {
        let (_dir, paths) = scratch("case", &["a.epub", "B.EPUB"]);
        assert_eq!(epub_from(&paths[..1]), Ok(paths[0].clone()));
        assert_eq!(epub_from(&paths[1..]), Ok(paths[1].clone()));
    }

    #[test]
    fn refuses_nothing() {
        assert_eq!(epub_from(&[]), Err("Nothing was dropped."));
    }

    #[test]
    fn refuses_several_files_even_if_one_is_an_epub() {
        let (_dir, paths) = scratch("several", &["a.epub", "notes.txt"]);
        assert_eq!(epub_from(&paths), Err("Drop one EPUB at a time."));
    }

    #[test]
    fn refuses_other_kinds() {
        let (_dir, paths) = scratch("kind", &["book.pdf", "noext"]);
        for path in paths {
            assert_eq!(epub_from(&[path]), Err("Only .epub files can be scanned."));
        }
    }

    #[test]
    fn refuses_a_folder_named_like_an_epub() {
        let (dir, _) = scratch("folder", &[]);
        let folder = dir.join("unpacked.epub");
        fs::create_dir_all(&folder).expect("folder");
        assert_eq!(epub_from(&[folder]), Err("Drop a file, not a folder."));
    }
}
