//! Project file-tree construction for the sidebar — a `FileNode` tree
//! built from the flat `project_manager.project.files` list.  Rendering
//! lives in `gui_sidebar.rs`.

#[derive(Debug, Clone)]
pub(crate) struct FileNode {
    pub(crate) name:     String,
    pub(crate) path:     String,
    pub(crate) is_dir:   bool,
    pub(crate) children: Vec<FileNode>,
}

/// Insert one indexed path (project-relative, `/`-separated) into the
/// tree.  Every node's `path` is the joined relative prefix, so node
/// identity matches `expanded_dirs` keys and the breadcrumb reveal.
fn insert(nodes: &mut Vec<FileNode>, parts: &[&str], prefix: &str) {
    if parts.is_empty() { return; }
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
            children: vec![],
        });
    } else {
        let mut dir = FileNode {
            name: name.to_string(),
            path: node_path.clone(),
            is_dir: true,
            children: vec![],
        };
        insert(&mut dir.children, &parts[1..], &node_path);
        nodes.push(dir);
    }
}

fn sort_nodes(nodes: &mut Vec<FileNode>) {
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _             => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    for n in nodes.iter_mut().filter(|n| n.is_dir) {
        sort_nodes(&mut n.children);
    }
}

pub(crate) fn build(files: &[String]) -> Vec<FileNode> {
    let mut roots: Vec<FileNode> = Vec::new();
    for path in files {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() { continue; }
        insert(&mut roots, &parts, "");
    }
    sort_nodes(&mut roots);
    roots
}

pub(crate) fn collect_dirs(nodes: &[FileNode], out: &mut std::collections::HashSet<String>) {
    for n in nodes {
        if n.is_dir {
            out.insert(n.path.clone());
            collect_dirs(&n.children, out);
        }
    }
}
