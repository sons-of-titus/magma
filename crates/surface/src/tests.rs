use super::*;

fn red() -> Style {
    Style { fg: (255, 0, 0), ..Style::default() }
}

#[test]
fn a_new_surface_is_all_blank_cells() {
    let s = Surface::new(4, 2);
    assert_eq!((s.width, s.height), (4, 2));
    for y in 0..2 {
        for x in 0..4 {
            let c = s.cell(x, y).unwrap();
            assert_eq!(c.ch, ' ');
            assert_eq!(c.style, Style::default());
        }
    }
    assert_eq!(s.rows().len(), 2);
    assert_eq!(s.rows()[0].len(), 4);
}

#[test]
fn writes_outside_the_grid_are_dropped() {
    let mut s = Surface::new(2, 2);
    s.set_cell(2, 0, 'x', None);
    s.set_cell(0, 2, 'x', None);
    s.set_text(1, 1, "ab", None);

    assert!(s.cell(2, 0).is_none());
    assert!(s.cell(0, 2).is_none());
    assert_eq!(s.cell(1, 1).unwrap().ch, 'a');
    assert_eq!(s.rows()[1][1].ch, 'a', "truncated at the row edge");
}

#[test]
fn text_advances_one_cell_per_char_and_keeps_existing_style_with_none() {
    let mut s = Surface::new(6, 1);
    s.set_cell(0, 0, 'Z', Some(red()));
    s.set_text(0, 0, "abc", None);

    assert_eq!(s.cell(0, 0).unwrap().ch, 'a');
    assert_eq!(s.cell(0, 0).unwrap().style, red(), "None keeps the style");
    assert_eq!(s.cell(2, 0).unwrap().ch, 'c');

    s.set_text(0, 0, "abc", Some(Style::default()));
    assert_eq!(s.cell(0, 0).unwrap().style, Style::default());
}

#[test]
fn clear_resets_characters_and_styles() {
    let mut s = Surface::new(3, 1);
    s.set_text(0, 0, "abc", Some(red()));
    s.clear();
    assert_eq!(s.rows()[0].iter().map(|c| c.ch).collect::<String>(), "   ");
    assert!(s.rows()[0].iter().all(|c| c.style == Style::default()));
}

#[test]
fn fill_style_paints_the_background_but_keeps_characters() {
    let mut s = Surface::new(2, 1);
    s.set_text(0, 0, "hi", None);
    let bg = Style { fg: (0, 0, 0), bg: (30, 30, 46), ..Style::default() };
    s.fill_style(bg);

    assert_eq!(s.rows()[0].iter().map(|c| c.ch).collect::<String>(), "hi");
    assert!(s.rows()[0].iter().all(|c| c.style.bg == (30, 30, 46)));
}

#[test]
fn resize_discards_content_and_is_a_no_op_when_unchanged() {
    let mut s = Surface::new(2, 2);
    s.set_text(0, 0, "ab", Some(red()));

    s.resize(2, 2);
    assert_eq!(s.cell(0, 0).unwrap().ch, 'a', "same size keeps the grid");

    s.resize(3, 1);
    assert_eq!((s.width, s.height), (3, 1));
    assert_eq!(s.cell(0, 0).unwrap().ch, ' ', "a resize starts blank");
    assert_eq!(s.rows().len(), 1);
    assert_eq!(s.rows()[0].len(), 3);
}
