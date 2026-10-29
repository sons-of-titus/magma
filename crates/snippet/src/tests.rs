use super::*;

fn numbers(stops: &[TabStop]) -> Vec<usize> {
    stops.iter().map(|s| s.number).collect()
}

fn expanded(stops: &[TabStop]) -> Vec<(usize, usize, &str)> {
    stops
        .iter()
        .map(|s| (s.number, s.offset, s.text.as_str()))
        .collect()
}

#[test]
fn plain_text_expands_to_itself_with_no_tabstops() {
    assert!(expand_lsp("").0.is_empty());
    let (text, stops) = expand_lsp("let x = 1;");
    assert_eq!(text, "let x = 1;");
    assert!(stops.is_empty());
    // A lone dollar is literal when it starts nothing.
    let (text, stops) = expand_lsp("price: 5$");
    assert_eq!(text, "price: 5$");
    assert!(stops.is_empty());
}

#[test]
fn dollar_n_is_a_tabstop_where_the_marker_stood() {
    let (text, stops) = expand_lsp("$1");
    assert_eq!(text, "");
    assert_eq!(expanded(&stops), vec![(1, 0, "")]);
}

#[test]
fn dollar_zero_is_the_final_cursor() {
    let (text, stops) = expand_lsp("return $0;");
    assert_eq!(text, "return ;");
    assert_eq!(expanded(&stops), vec![(0, 7, "")]);
}

#[test]
fn brace_form_keeps_the_placeholder_text() {
    let (text, stops) = expand_lsp("${1:name}");
    assert_eq!(text, "name");
    assert_eq!(expanded(&stops), vec![(1, 0, "name")]);
}

#[test]
fn brace_form_without_text_has_an_empty_placeholder() {
    let (text, stops) = expand_lsp("${2}");
    assert_eq!(text, "");
    assert_eq!(expanded(&stops), vec![(2, 0, "")]);
}

#[test]
fn placeholder_offsets_point_into_the_expanded_text() {
    let (text, stops) = expand_lsp("let x = ${1:x};");
    assert_eq!(text, "let x = x;");
    assert_eq!(expanded(&stops), vec![(1, 8, "x")]);
    let (text, stops) = expand_lsp("impl ${1:Name} {\n    ${0}\n}");
    assert_eq!(text, "impl Name {\n    \n}");
    let first = &stops[0];
    assert_eq!(&text[first.offset..first.offset + first.text_len], "Name");
}

#[test]
fn mirrors_are_listed_once_per_occurrence() {
    let (text, stops) = expand_lsp("${1:a} + ${1:a}");
    assert_eq!(text, "a + a");
    assert_eq!(expanded(&stops), vec![(1, 0, "a"), (1, 4, "a")]);
}

#[test]
fn stops_are_returned_in_occurrence_order() {
    let (text, stops) = expand_lsp("${2:n} ${0} ${1:i}");
    assert_eq!(text, "n  i");
    assert_eq!(numbers(&stops), vec![2, 0, 1]);
    assert!(stops[0].offset < stops[1].offset);
    assert!(stops[1].offset < stops[2].offset);
}

#[test]
fn dollar_dollar_is_a_literal_dollar() {
    let (text, stops) = expand_lsp("price: $$${1:n}");
    assert_eq!(text, "price: $n");
    assert_eq!(expanded(&stops), vec![(1, 8, "n")]);
}

#[test]
fn the_if_builtin_expands_with_a_condition_and_a_final_stop() {
    let expansion = expand("if").expect("if is a builtin");
    let (text, stops) = expand_lsp(&expansion);
    assert!(text.starts_with("if ("));
    assert!(!text.contains('$'), "no marker survives expansion: {text:?}");
    assert_eq!(numbers(&stops), vec![1, 0]);
    assert_eq!(stops[0].text, "condition");
}

#[test]
fn unknown_triggers_do_not_expand() {
    assert!(expand("nope").is_none());
    assert!(expand("").is_none());
}

#[test]
fn every_builtin_expansion_is_marker_free_with_tabstops() {
    let snippets = builtin();
    assert_eq!(snippets.len(), 14);
    for (trigger, expansion) in &snippets {
        let (text, stops) = expand_lsp(expansion);
        assert!(!text.contains('$'), "builtin {trigger:?} left a marker");
        assert!(!stops.is_empty(), "builtin {trigger:?} produced no tab stops");
    }
}

#[test]
fn the_for_loop_builtin_mirrors_its_index_three_times() {
    let (_, stops) = expand_lsp(&expand("for").unwrap());
    assert_eq!(numbers(&stops), vec![1, 1, 2, 1, 0]);
}
