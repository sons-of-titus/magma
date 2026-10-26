use super::*;

fn numbers(stops: &[TabStop]) -> Vec<usize> {
    stops.iter().map(|s| s.number).collect()
}

#[test]
fn plain_text_has_no_tabstops() {
    assert!(parse_tabstops("").is_empty());
    assert!(parse_tabstops("let x = 1;").is_empty());
    assert!(parse_tabstops("price: 5$").is_empty());
}

#[test]
fn dollar_n_is_a_tabstop_starting_at_the_dollar() {
    let stops = parse_tabstops("$1");
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].number, 1);
    assert_eq!(stops[0].offset, 0);
    assert!(stops[0].text.is_empty());
    assert_eq!(stops[0].text_len, 2);
}

#[test]
fn dollar_zero_is_the_final_cursor() {
    let stops = parse_tabstops("return $0;");
    assert_eq!(numbers(&stops), vec![0]);
    assert_eq!(stops[0].offset, 7);
}

#[test]
fn brace_form_keeps_the_placeholder_text() {
    let stops = parse_tabstops("${1:name}");
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].number, 1);
    assert_eq!(stops[0].offset, 0);
    assert_eq!(stops[0].text, "name");
}

#[test]
fn brace_form_without_text_has_an_empty_placeholder() {
    let stops = parse_tabstops("${2}");
    assert_eq!(numbers(&stops), vec![2]);
    assert!(stops[0].text.is_empty());
    assert_eq!(stops[0].text_len, 0);
}

#[test]
fn brace_form_after_prefix_reports_the_prefix_length_as_offset() {
    let stops = parse_tabstops("let x = ${1:x};");
    assert_eq!(numbers(&stops), vec![1]);
    assert_eq!(stops[0].offset, 8);
    assert_eq!(stops[0].text, "x");
}

#[test]
fn mirrors_are_listed_once_per_occurrence() {
    let stops = parse_tabstops("${1:a} + ${1:a}");
    assert_eq!(numbers(&stops), vec![1, 1]);
    assert_eq!(stops[0].offset, 0);
    assert_eq!(stops[1].offset, 9);
}

#[test]
fn stops_are_returned_in_occurrence_order() {
    let stops = parse_tabstops("${2:n} ${0} ${1:i}");
    assert_eq!(numbers(&stops), vec![2, 0, 1]);
    assert!(stops[0].offset < stops[1].offset);
    assert!(stops[1].offset < stops[2].offset);
}

#[test]
fn the_if_builtin_expands_with_a_condition_and_a_final_stop() {
    let expansion = expand("if").expect("if is a builtin");
    assert!(expansion.starts_with("if ("));
    let stops = parse_tabstops(&expansion);
    assert_eq!(numbers(&stops), vec![1, 0]);
    assert_eq!(stops[0].text, "condition");
}

#[test]
fn unknown_triggers_do_not_expand() {
    assert!(expand("nope").is_none());
    assert!(expand("").is_none());
}

#[test]
fn every_builtin_expansion_parses_into_tabstops() {
    let snippets = builtin();
    assert_eq!(snippets.len(), 14);
    for (trigger, expansion) in &snippets {
        let stops = parse_tabstops(expansion);
        assert!(
            !stops.is_empty(),
            "builtin {trigger:?} produced no tab stops"
        );
    }
}

#[test]
fn the_for_loop_builtin_mirrors_its_index_three_times() {
    let stops = parse_tabstops(&expand("for").unwrap());
    assert_eq!(numbers(&stops), vec![1, 1, 2, 1, 0]);
}
