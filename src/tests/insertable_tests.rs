use crate::input::is_insertable;

#[test]
fn regular_letters_are_insertable() {
    for ch in "abcdefghijklmnopqrstuvwxyz".chars() {
        let s = ch.to_string();
        assert!(is_insertable(&s), "{s:?} should be insertable");
    }
}

#[test]
fn uppercase_letters_are_insertable() {
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
        let s = ch.to_string();
        assert!(is_insertable(&s), "{s:?} should be insertable");
    }
}

#[test]
fn digits_are_insertable() {
    for ch in "0123456789".chars() {
        let s = ch.to_string();
        assert!(is_insertable(&s), "{s:?} should be insertable");
    }
}

#[test]
fn symbols_are_insertable() {
    for s in ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "-", "=", "+", " "] {
        assert!(is_insertable(s), "{s:?} should be insertable");
    }
}

#[test]
fn f_letter_is_insertable() {
    assert!(is_insertable("f"), "bare 'f' must be insertable (vim motion)");
}

#[test]
fn function_keys_not_insertable() {
    for k in ["f1","f2","f3","f4","f5","f6","f7","f8","f9","f10","f11","f12"] {
        assert!(!is_insertable(k), "{k:?} should not be insertable");
    }
}

#[test]
fn named_keys_not_insertable() {
    for k in ["esc","backspace","delete","tab","return",
              "left","right","up","down","home","end","page-up","page-down"] {
        assert!(!is_insertable(k), "{k:?} should not be insertable");
    }
}

#[test]
fn ctrl_meta_not_insertable() {
    assert!(!is_insertable("ctrl-s"));
    assert!(!is_insertable("ctrl-q"));
    assert!(!is_insertable("meta-x"));
}

#[test]
fn control_chars_not_insertable() {
    assert!(!is_insertable("\x00"));
    assert!(!is_insertable("\x1b"));
    assert!(!is_insertable("\x7f"));
}
