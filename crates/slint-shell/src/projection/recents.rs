//! Welcome-screen rows for recent projects.

/// Folder name and a display path with the home directory shortened to `~`.
pub fn split(path: &str, home: Option<&str>) -> (String, String) {
    let trimmed = path.trim_end_matches('/');
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let name = if name.is_empty() { trimmed } else { name };
    let display = match home {
        Some(h) if !h.is_empty() && trimmed.starts_with(h) && trimmed[h.len()..].starts_with('/') => {
            format!("~{}", &trimmed[h.len()..])
        }
        _ => trimmed.to_string(),
    };
    (name.to_string(), display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_name_and_shortens_home() {
        assert_eq!(
            split("/Users/me/Projects/magma/", Some("/Users/me")),
            ("magma".into(), "~/Projects/magma".into())
        );
    }

    #[test]
    fn leaves_paths_outside_home_alone() {
        assert_eq!(
            split("/opt/app", Some("/Users/me")),
            ("app".into(), "/opt/app".into())
        );
    }

    #[test]
    fn home_prefix_must_end_at_a_separator() {
        assert_eq!(
            split("/Users/meow/x", Some("/Users/me")),
            ("x".into(), "/Users/meow/x".into())
        );
    }
}
