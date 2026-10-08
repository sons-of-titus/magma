//! LineIndex contract — newline structure ownership (docs/core-redesign.md §1.4/§A.5).

use crate::kernel::text_engine::document::Document;
use crate::kernel::text_engine::transaction::TransactionBuilder;

#[test]
fn empty_document_has_one_line() {
    let doc = Document::new();
    let li = doc.line_index();
    assert_eq!(li.line_count(), 1);
    assert_eq!(li.last_line(), 0);
    assert_eq!(li.line_start(0), Some(0));
    assert_eq!(li.line_end(0), Some(0));
    assert_eq!(li.line_span(0), Some(0..0));
    assert_eq!(li.line_start(1), None);
    assert_eq!(li.line_end(1), None);
}

#[test]
fn no_trailing_newline() {
    let doc = Document::from_string("abc\ndefg\nhi");
    let li = doc.line_index();
    assert_eq!(li.line_count(), 3);
    assert_eq!(li.last_line(), 2);
    assert_eq!(li.line_start(0), Some(0));
    assert_eq!(li.line_start(1), Some(4));
    assert_eq!(li.line_start(2), Some(9));
    assert_eq!(li.line_end(0), Some(3));   // position of the '\n'
    assert_eq!(li.line_end(1), Some(8));
    assert_eq!(li.line_end(2), Some(11));  // final line ends at document end
    assert_eq!(li.line_span(0), Some(0..3));
    assert_eq!(li.line_span(2), Some(9..11));
}

#[test]
fn trailing_newline_begins_a_final_empty_line() {
    let doc = Document::from_string("a\nb\n");
    let li = doc.line_index();
    assert_eq!(li.line_count(), 3);
    assert_eq!(li.line_span(2), Some(4..4));
}

#[test]
fn newline_only_document() {
    let doc = Document::from_string("\n");
    let li = doc.line_index();
    assert_eq!(li.line_count(), 2);
    assert_eq!(li.line_span(0), Some(0..0));
    assert_eq!(li.line_span(1), Some(1..1));
}

#[test]
fn byte_at_newline_belongs_to_the_line_it_terminates() {
    let doc = Document::from_string("abc\ndefg\nhi");
    let li = doc.line_index();
    assert_eq!(li.line_of(0), 0);
    assert_eq!(li.line_of(2), 0);
    assert_eq!(li.line_of(3), 0);   // the '\n'
    assert_eq!(li.line_of(4), 1);   // first byte after it
    assert_eq!(li.line_of(8), 1);
    assert_eq!(li.line_of(9), 2);
    assert_eq!(li.line_of(11), 2);  // EOF
}

#[test]
fn line_of_clamps_past_the_end() {
    let doc = Document::from_string("a\nb");
    let li = doc.line_index();
    assert_eq!(li.line_of(3), 1);
    assert_eq!(li.line_of(999), 1);
}

#[test]
fn carriage_return_belongs_to_line_content() {
    let doc = Document::from_string("a\r\nb\r\n");
    let li = doc.line_index();
    assert_eq!(li.line_count(), 3);
    assert_eq!(li.line_span(0), Some(0..2));  // "a\r" — '\r' is content
    assert_eq!(li.line_of(1), 0);             // the '\r'
    assert_eq!(li.line_of(2), 0);             // the '\n'
    assert_eq!(li.line_of(3), 1);
}

#[test]
fn multibyte_lines_index_by_bytes_not_chars() {
    // h=0, é=1..3, l=3, l=4, o=5, \n=6, w=7…
    let doc = Document::from_string("héllo\nwörld");
    let li = doc.line_index();
    assert_eq!(li.line_count(), 2);
    assert_eq!(li.line_of(1), 0);
    assert_eq!(li.line_of(6), 0);
    assert_eq!(li.line_of(7), 1);
    assert_eq!(li.line_start(1), Some(7));
    assert_eq!(li.line_end(0), Some(6));
    assert_eq!(&doc.text()[li.line_span(0).unwrap()], "héllo");
    assert_eq!(&doc.text()[li.line_span(1).unwrap()], "wörld");
}

#[test]
fn line_index_tracks_edits_without_rebuild() {
    let mut doc = Document::from_string("a\nb");
    assert_eq!(doc.line_index().line_count(), 2);

    doc.commit(TransactionBuilder::new().insert(1, "\n")).unwrap().unwrap();
    assert_eq!(doc.text(), "a\n\nb");
    assert_eq!(doc.line_index().line_count(), 3);
    assert_eq!(doc.line_index().line_of(2), 1);

    doc.commit(TransactionBuilder::new().delete(0..2)).unwrap().unwrap();
    assert_eq!(doc.text(), "\nb");
    assert_eq!(doc.line_index().line_count(), 2);
    assert_eq!(doc.line_index().line_of(0), 0);
    assert_eq!(doc.line_index().line_of(1), 1);
}
