//! Bounded graph views over the saved response; navigation keeps stable node paths.
use crate::api::TreeNode;
use serde_json::Value;

pub(super) const PAGE_SIZE: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Focus {
    #[default]
    Book,
    Json(String),
    Structure(Option<String>),
}

impl Focus {
    pub fn parent(&self) -> Self {
        match self {
            Self::Book | Self::Structure(None) => Self::Book,
            Self::Json(path) => match path.rsplit_once('/') {
                Some((parent, _)) if !parent.is_empty() => Self::Json(parent.into()),
                _ => Self::Book,
            },
            Self::Structure(Some(id)) => {
                Self::Structure(id.rsplit_once('.').map(|(parent, _)| parent.to_string()))
            }
        }
    }
    pub fn key(&self) -> String {
        match self {
            Self::Book => "book".into(),
            Self::Json(path) => path.clone(),
            Self::Structure(id) => format!("structure/{}", id.as_deref().unwrap_or("root")),
        }
    }
}

#[derive(Clone)]
pub(super) struct Node {
    pub label: String,
    pub detail: String,
    pub group: usize,
    pub target: Focus,
}

pub(super) struct View {
    pub nodes: Vec<Node>,
    pub edges: Vec<(usize, usize)>,
    pub page: usize,
    pub pages: usize,
    pub total: usize,
}

fn node(label: impl Into<String>, group: usize, target: Focus) -> Node {
    Node {
        label: label.into(),
        detail: String::new(),
        group,
        target,
    }
}
fn short(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(_) => "Details".into(),
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}
pub(super) fn graph_label(label: &str) -> String {
    if label.chars().count() > 36 {
        format!("{}…", label.chars().take(35).collect::<String>())
    } else {
        label.into()
    }
}
fn pointer_child(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}

pub(super) fn tree_node<'a>(tree: &'a [TreeNode], id: &str) -> Option<&'a TreeNode> {
    let mut indices = id.strip_prefix('n')?.split('.').map(str::parse::<usize>);
    let mut node = tree.get(indices.next()?.ok()?)?;
    for index in indices {
        node = node.children.get(index.ok()?)?;
    }
    Some(node)
}

pub(super) fn structure_value<'a>(value: &'a Value, id: &str) -> Option<&'a Value> {
    let mut indices = id.strip_prefix('n')?.split('.').map(str::parse::<usize>);
    let mut node = value["structure"]["nodes"].get(indices.next()?.ok()?)?;
    for index in indices {
        let children = ["children", "paragraphs", "sentences", "clauses", "words"]
            .into_iter()
            .find_map(|key| node[key].as_array())?;
        node = children.get(index.ok()?)?;
    }
    Some(node)
}

pub(super) fn inspected<'a>(value: &'a Value, focus: &Focus) -> Option<&'a Value> {
    match focus {
        Focus::Book => value.get("book"),
        Focus::Json(path) => value.pointer(path),
        Focus::Structure(Some(id)) => structure_value(value, id),
        Focus::Structure(None) => value.get("structure"),
    }
}

fn structural_node(node: &TreeNode) -> Node {
    let depth = node.id.matches('.').count();
    Node {
        label: node.label.clone(),
        detail: node.meta.clone(),
        group: (4 + depth).min(7),
        target: Focus::Structure(Some(node.id.clone())),
    }
}

pub(super) fn view(value: &Value, tree: &[TreeNode], focus: &Focus, requested_page: usize) -> View {
    let book_title = value["book"]["title"].as_str().unwrap_or("Book");
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut children: Box<dyn Iterator<Item = Node> + '_> = Box::new(std::iter::empty());
    let mut total = 0;
    let anchor;
    match focus {
        Focus::Book => {
            nodes.push(node(book_title, 0, Focus::Book));
            let mut add = |label: String, group, target| {
                let index = nodes.len();
                nodes.push(node(label, group, target));
                edges.push((0, index));
            };
            add("Metadata".into(), 1, Focus::Json("/book".into()));
            for key in ["authors", "language", "publisher", "license"] {
                let field = &value["book"][key];
                if !field.is_null() && field.as_array().is_none_or(|items| !items.is_empty()) {
                    let label = if key == "authors" {
                        field
                            .as_array()
                            .filter(|a| a.len() == 1)
                            .and_then(|a| a[0]["name"].as_str())
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("Authors · {}", short(field)))
                    } else {
                        format!("{key} · {}", short(field))
                    };
                    add(label, 1, Focus::Json(format!("/book/{key}")));
                }
            }
            add(
                format!("Schema · {}", short(&value["structure"]["schema"])),
                2,
                Focus::Json("/structure".into()),
            );
            add(
                "Counts & averages".into(),
                3,
                Focus::Json("/structure/summary".into()),
            );
            if value["meta"].is_object() {
                add("Scan & source".into(), 1, Focus::Json("/meta".into()));
            }
            anchor = nodes.len();
            nodes.push(node(
                format!("Structure · {} top-level nodes", tree.len()),
                4,
                Focus::Structure(None),
            ));
            edges.push((0, anchor));
            total = tree.len();
            children = Box::new(tree.iter().map(structural_node));
        }
        Focus::Structure(id) => {
            let selected = id.as_deref().and_then(|id| tree_node(tree, id));
            nodes.push(
                selected
                    .map(structural_node)
                    .unwrap_or_else(|| node(book_title, 0, focus.clone())),
            );
            anchor = 0;
            if let Some(selected) = selected {
                let parent = focus.parent();
                let label = match &parent {
                    Focus::Structure(Some(id)) => tree_node(tree, id)
                        .map(|n| n.label.as_str())
                        .unwrap_or(book_title),
                    _ => book_title,
                };
                nodes.push(node(label, 0, parent));
                edges.push((0, 1));
                total = selected.children.len();
                children = Box::new(selected.children.iter().map(structural_node));
            } else {
                total = tree.len();
                children = Box::new(tree.iter().map(structural_node));
            }
        }
        Focus::Json(path) => {
            let data = value.pointer(path).unwrap_or(&Value::Null);
            let title = path.rsplit('/').next().unwrap_or("Metadata");
            nodes.push(node(
                title,
                if path.starts_with("/book") { 1 } else { 2 },
                focus.clone(),
            ));
            anchor = 0;
            let group = if path.contains("summary") {
                3
            } else if path.starts_with("/structure") {
                2
            } else {
                1
            };
            match data {
                Value::Object(fields) => {
                    total = fields.values().filter(|v| !v.is_null()).count();
                    children = Box::new(fields.iter().filter(|(_, v)| !v.is_null()).map(
                        move |(key, v)| {
                            let target = if path == "/structure" && key == "nodes" {
                                Focus::Structure(None)
                            } else {
                                Focus::Json(pointer_child(path, key))
                            };
                            node(format!("{key} · {}", short(v)), group, target)
                        },
                    ))
                }
                Value::Array(items) => {
                    total = items.len();
                    children = Box::new(items.iter().enumerate().map(move |(ix, v)| {
                        let label = v["name"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| short(v));
                        node(
                            label,
                            group,
                            Focus::Json(pointer_child(path, &ix.to_string())),
                        )
                    }));
                }
                _ => nodes[0].detail = short(data),
            }
        }
    }
    let pages = total.div_ceil(PAGE_SIZE).max(1);
    let page = requested_page.min(pages - 1);
    for child in children.skip(page * PAGE_SIZE).take(PAGE_SIZE) {
        let index = nodes.len();
        nodes.push(child);
        edges.push((anchor, index));
    }
    View {
        nodes,
        edges,
        page,
        pages,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::build_tree;
    use serde_json::json;
    #[test]
    fn pages_keep_all_chapters_and_valid_edges() {
        let value = json!({"book":{"title":"Large book","authors":[{"name":"Author"}],"language":"en"},
            "structure":{"schema":"standard_book","nodes":(1..=61).map(|i| json!({"level":"chapter","index":i,"label":format!("Chapter {i}")})).collect::<Vec<_>>()}});
        let tree = build_tree(&value["structure"]);
        let mut seen = Vec::new();
        for page in 0..6 {
            let graph = view(&value, &tree, &Focus::Book, page);
            assert!(graph.nodes.len() <= 24);
            assert_eq!(graph.total, 61);
            assert!(graph
                .edges
                .iter()
                .all(|(a, b)| *a < graph.nodes.len() && *b < graph.nodes.len()));
            seen.extend(graph.nodes.iter().filter_map(|n| match &n.target {
                Focus::Structure(Some(id)) => Some(id.clone()),
                _ => None,
            }));
        }
        assert_eq!(seen.len(), 61);
        assert_eq!(seen.last().unwrap(), "n60");
    }
    #[test]
    fn structure_drilldown_preserves_paths_and_parent() {
        let value = json!({"book":{"title":"Book"},"structure":{"nodes":[{"level":"chapter","index":1,"paragraph_count":1,"paragraphs":[{"index":1,"word_count":9,"sentences":[{"index":1,"word_count":9}]}]}]}});
        let tree = build_tree(&value["structure"]);
        let focus = Focus::Structure(Some("n0.0".into()));
        let graph = view(&value, &tree, &focus, 0);
        assert_eq!(graph.total, 1);
        assert_eq!(
            graph.nodes.last().unwrap().target,
            Focus::Structure(Some("n0.0.0".into()))
        );
        assert_eq!(focus.parent(), Focus::Structure(Some("n0".into())));
        assert_eq!(inspected(&value, &focus).unwrap()["word_count"], 9);
    }
    #[test]
    fn metadata_relationships_keep_multiple_authors_and_escaped_keys() {
        let value = json!({"book":{"authors":[{"name":"First"},{"name":"Second"}],"publisher":"Press","custom/key":"value"},"structure":{"nodes":[]}});
        let authors = view(&value, &[], &Focus::Json("/book/authors".into()), 0);
        assert_eq!(authors.total, 2);
        assert_eq!(authors.nodes[2].label, "Second");
        assert_eq!(
            inspected(&value, &authors.nodes[2].target).unwrap()["name"],
            "Second"
        );
        let metadata = view(&value, &[], &Focus::Json("/book".into()), 0);
        let custom = metadata
            .nodes
            .iter()
            .find(|n| n.label.starts_with("custom/key"))
            .unwrap();
        assert_eq!(inspected(&value, &custom.target).unwrap(), "value");
    }
}
