//! Project file tree — a pure projection from the flat indexed file list.
//!
//! Both frontends render the same tree: `build` turns project-relative paths
//! into nodes, `visible_rows` flattens it through the caller's expanded set.
//! No widget, editor, or filesystem state lives here.

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub ignored: bool,
    pub children: Vec<FileNode>,
}

/// One visible row, ready for a frontend list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub name: String,
    pub path: String,
    pub depth: usize,
    pub is_dir: bool,
    pub ignored: bool,
    pub expanded: bool,
}

fn insert(nodes: &mut Vec<FileNode>, parts: &[&str], prefix: &str) {
    if parts.is_empty() {
        return;
    }
    let name = parts[0];
    let is_leaf = parts.len() == 1;
    let node_path = if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", prefix, name)
    };

    if let Some(existing) = nodes.iter_mut().find(|n| n.name == name) {
        if !is_leaf {
            insert(&mut existing.children, &parts[1..], &node_path);
        }
    } else if is_leaf {
        nodes.push(FileNode {
            name: name.to_string(),
            path: node_path,
            is_dir: false,
            ignored: false,
            children: vec![],
        });
    } else {
        let mut dir = FileNode {
            name: name.to_string(),
            path: node_path.clone(),
            is_dir: true,
            ignored: false,
            children: vec![],
        };
        insert(&mut dir.children, &parts[1..], &node_path);
        nodes.push(dir);
    }
}

fn sort_nodes(nodes: &mut [FileNode]) {
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    for n in nodes.iter_mut().filter(|n| n.is_dir) {
        sort_nodes(&mut n.children);
    }
}

pub fn build(files: &[String], ignored: &HashSet<String>) -> Vec<FileNode> {
    let mut roots: Vec<FileNode> = Vec::new();
    for path in files {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        insert(&mut roots, &parts, "");
    }
    // Mark ignored nodes.
    fn mark(nodes: &mut [FileNode], ignored: &HashSet<String>) {
        for n in nodes.iter_mut() {
            if ignored.contains(&n.path) {
                n.ignored = true;
            }
            mark(&mut n.children, ignored);
        }
    }
    mark(&mut roots, ignored);
    sort_nodes(&mut roots);
    roots
}

pub fn collect_dirs(nodes: &[FileNode], out: &mut HashSet<String>) {
    for n in nodes {
        if n.is_dir {
            out.insert(n.path.clone());
            collect_dirs(&n.children, out);
        }
    }
}

/// Flatten the tree to the rows currently visible under `expanded`.
pub fn visible_rows(nodes: &[FileNode], expanded: &HashSet<String>) -> Vec<TreeRow> {
    fn walk(nodes: &[FileNode], depth: usize, expanded: &HashSet<String>, out: &mut Vec<TreeRow>) {
        for node in nodes {
            let open = node.is_dir && expanded.contains(&node.path);
            out.push(TreeRow {
                name: node.name.clone(),
                path: node.path.clone(),
                depth,
                is_dir: node.is_dir,
                ignored: node.ignored,
                expanded: open,
            });
            if open {
                walk(&node.children, depth + 1, expanded, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, 0, expanded, &mut out);
    out
}

/// Split a path into display segments: `src/kernel/mod.rs` -> `["src", "kernel", "mod.rs"]`.
pub fn segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Strip `root` off the front of `path`, returning a project-relative path.
pub fn relative_to_project(root: Option<&std::path::Path>, path: &str) -> String {
    if let Some(root) = root {
        let root_str = root.to_string_lossy().trim_end_matches('/').to_string();
        if let Some(stripped) = path.strip_prefix(&root_str) {
            return stripped.trim_start_matches('/').to_string();
        }
    }
    path.to_string()
}
