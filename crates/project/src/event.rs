//! The project subsystem's background-event vocabulary.
//!
//! Find-files walks and project-wide content searches produce these off the
//! foreground thread; the editor wraps them in its own transport enum so the
//! foreground keeps one ordered channel while the project owns the payloads.

use tool_diag::QuickfixEntry;

/// An event produced by a project-wide file walk or content search.
#[derive(Debug, Clone)]
pub enum Event {
    /// A find-files walk finished; the foreground presents the results.
    FileSearchListed { pattern: String, files: Vec<String> },
    /// A streamed batch of project-search results; appended to the quickfix
    /// list when the generation is still current.
    GrepResultsBatch {
        generation: u64,
        entries: Vec<QuickfixEntry>,
    },
    /// A project-wide search finished; the quickfix index resets.
    GrepDone { generation: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(filename: &str, line: usize) -> QuickfixEntry {
        QuickfixEntry {
            filename: filename.into(),
            line,
            col: 1,
            message: "hit".into(),
            ..Default::default()
        }
    }

    #[test]
    fn event_payloads_carry_walk_and_search_results() {
        let listed = Event::FileSearchListed {
            pattern: "main".into(),
            files: vec!["src/main.rs".into()],
        };
        match listed {
            Event::FileSearchListed { pattern, files } => {
                assert_eq!(pattern, "main");
                assert_eq!(files, vec!["src/main.rs".to_string()]);
            }
            _ => panic!("wrong variant"),
        }

        let batch = Event::GrepResultsBatch {
            generation: 3,
            entries: vec![entry("src/lib.rs", 9)],
        };
        match batch.clone() {
            Event::GrepResultsBatch {
                generation,
                entries,
            } => {
                assert_eq!(generation, 3);
                assert_eq!(entries[0].line, 9);
            }
            _ => panic!("wrong variant"),
        }

        assert!(matches!(
            Event::GrepDone { generation: 3 },
            Event::GrepDone { .. }
        ));
    }
}
