//! Project tree projection — nesting, ordering, and expansion.

use std::collections::HashSet;

use crate::kernel::project::tree::{build, collect_dirs, visible_rows};

fn files() -> Vec<String> {
    vec![
        "src/main.rs".into(),
        "src/kernel/mod.rs".into(),
        "Cargo.toml".into(),
        "docs/readme.md".into(),
    ]
}

#[test]
fn tree_nests_and_sorts_dirs_before_files() {
    let tree = build(&files());
    assert_eq!(view_names(&tree), vec!["docs", "src", "Cargo.toml"]);
    let src = tree.iter().find(|n| n.name == "src").unwrap();
    assert!(src.is_dir);
    assert_eq!(view_names(&src.children), vec!["kernel", "main.rs"]);
    let kernel = src.children.iter().find(|n| n.name == "kernel").unwrap();
    assert_eq!(kernel.path, "src/kernel");
    assert_eq!(kernel.children[0].path, "src/kernel/mod.rs");
}

#[test]
fn visible_rows_respect_collapsed_directories() {
    let tree = build(&files());
    let mut expanded = HashSet::new();

    let collapsed = visible_rows(&tree, &expanded);
    assert_eq!(row_paths(&collapsed), vec!["docs", "src", "Cargo.toml"]);
    assert!(collapsed.iter().all(|r| !r.expanded));

    expanded.insert("src".to_string());
    let open = visible_rows(&tree, &expanded);
    assert_eq!(
        row_paths(&open),
        vec!["docs", "src", "src/kernel", "src/main.rs", "Cargo.toml"]
    );
    let kernel = open.iter().find(|r| r.path == "src/kernel").unwrap();
    assert_eq!(kernel.depth, 1);
    assert!(!kernel.expanded, "kernel is still collapsed");

    let mut all = HashSet::new();
    collect_dirs(&tree, &mut all);
    assert!(all.contains("src") && all.contains("src/kernel") && all.contains("docs"));
    assert!(all.iter().all(|p| !p.ends_with(".rs")));
}

fn view_names(nodes: &[crate::kernel::project::tree::FileNode]) -> Vec<&str> {
    nodes.iter().map(|n| n.name.as_str()).collect()
}

fn row_paths(rows: &[crate::kernel::project::tree::TreeRow]) -> Vec<&str> {
    rows.iter().map(|r| r.path.as_str()).collect()
}
