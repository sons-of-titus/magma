//! Contextual Arabic letter shaping into Unicode Presentation Forms.
//!
//! Operates in-place on `RowLayout::leads` in logical order before bidi
//! visual reordering, so each grapheme cell keeps its 1:1 offset and width
//! while rendering connected initial, medial, final, or isolated glyphs.

#[derive(Clone, Copy)]
enum Joining {
    Right([char; 2]),
    Dual([char; 4]),
    Tatweel,
}

impl Joining {
    fn joins_right(self) -> bool {
        true
    }

    fn joins_left(self) -> bool {
        matches!(self, Self::Dual(_) | Self::Tatweel)
    }

    fn select(self, join_prev: bool, join_next: bool) -> Option<char> {
        match self {
            Self::Right([isolated, final_form]) => {
                Some(if join_prev { final_form } else { isolated })
            }
            Self::Dual([isolated, final_form, initial, medial]) => {
                Some(match (join_prev, join_next) {
                    (false, false) => isolated,
                    (true, false) => final_form,
                    (false, true) => initial,
                    (true, true) => medial,
                })
            }
            Self::Tatweel => None,
        }
    }
}

fn classify(ch: char) -> Option<Joining> {
    use Joining::{Dual, Right, Tatweel};
    let j = match ch {
        '\u{0622}' => Right(['\u{FE81}', '\u{FE82}']),
        '\u{0623}' => Right(['\u{FE83}', '\u{FE84}']),
        '\u{0624}' => Right(['\u{FE85}', '\u{FE86}']),
        '\u{0625}' => Right(['\u{FE87}', '\u{FE88}']),
        '\u{0626}' => Dual(['\u{FE89}', '\u{FE8A}', '\u{FE8B}', '\u{FE8C}']),
        '\u{0627}' => Right(['\u{FE8D}', '\u{FE8E}']),
        '\u{0628}' => Dual(['\u{FE8F}', '\u{FE90}', '\u{FE91}', '\u{FE92}']),
        '\u{0629}' => Right(['\u{FE93}', '\u{FE94}']),
        '\u{062A}' => Dual(['\u{FE95}', '\u{FE96}', '\u{FE97}', '\u{FE98}']),
        '\u{062B}' => Dual(['\u{FE99}', '\u{FE9A}', '\u{FE9B}', '\u{FE9C}']),
        '\u{062C}' => Dual(['\u{FE9D}', '\u{FE9E}', '\u{FE9F}', '\u{FEA0}']),
        '\u{062D}' => Dual(['\u{FEA1}', '\u{FEA2}', '\u{FEA3}', '\u{FEA4}']),
        '\u{062E}' => Dual(['\u{FEA5}', '\u{FEA6}', '\u{FEA7}', '\u{FEA8}']),
        '\u{062F}' => Right(['\u{FEA9}', '\u{FEAA}']),
        '\u{0630}' => Right(['\u{FEAB}', '\u{FEAC}']),
        '\u{0631}' => Right(['\u{FEAD}', '\u{FEAE}']),
        '\u{0632}' => Right(['\u{FEAF}', '\u{FEB0}']),
        '\u{0633}' => Dual(['\u{FEB1}', '\u{FEB2}', '\u{FEB3}', '\u{FEB4}']),
        '\u{0634}' => Dual(['\u{FEB5}', '\u{FEB6}', '\u{FEB7}', '\u{FEB8}']),
        '\u{0635}' => Dual(['\u{FEB9}', '\u{FEBA}', '\u{FEBB}', '\u{FEBC}']),
        '\u{0636}' => Dual(['\u{FEBD}', '\u{FEBE}', '\u{FEBF}', '\u{FEC0}']),
        '\u{0637}' => Dual(['\u{FEC1}', '\u{FEC2}', '\u{FEC3}', '\u{FEC4}']),
        '\u{0638}' => Dual(['\u{FEC5}', '\u{FEC6}', '\u{FEC7}', '\u{FEC8}']),
        '\u{0639}' => Dual(['\u{FEC9}', '\u{FECA}', '\u{FECB}', '\u{FECC}']),
        '\u{063A}' => Dual(['\u{FECD}', '\u{FECE}', '\u{FECF}', '\u{FED0}']),
        '\u{0640}' => Tatweel,
        '\u{0641}' => Dual(['\u{FED1}', '\u{FED2}', '\u{FED3}', '\u{FED4}']),
        '\u{0642}' => Dual(['\u{FED5}', '\u{FED6}', '\u{FED7}', '\u{FED8}']),
        '\u{0643}' => Dual(['\u{FED9}', '\u{FEDA}', '\u{FEDB}', '\u{FEDC}']),
        '\u{0644}' => Dual(['\u{FEDD}', '\u{FEDE}', '\u{FEDF}', '\u{FEE0}']),
        '\u{0645}' => Dual(['\u{FEE1}', '\u{FEE2}', '\u{FEE3}', '\u{FEE4}']),
        '\u{0646}' => Dual(['\u{FEE5}', '\u{FEE6}', '\u{FEE7}', '\u{FEE8}']),
        '\u{0647}' => Dual(['\u{FEE9}', '\u{FEEA}', '\u{FEEB}', '\u{FEEC}']),
        '\u{0648}' => Right(['\u{FEED}', '\u{FEEE}']),
        '\u{0649}' => Right(['\u{FEEF}', '\u{FEF0}']),
        '\u{064A}' => Dual(['\u{FEF1}', '\u{FEF2}', '\u{FEF3}', '\u{FEF4}']),
        '\u{067E}' => Dual(['\u{FB56}', '\u{FB57}', '\u{FB58}', '\u{FB59}']),
        '\u{0686}' => Dual(['\u{FB7A}', '\u{FB7B}', '\u{FB7C}', '\u{FB7D}']),
        '\u{0698}' => Right(['\u{FB8A}', '\u{FB8B}']),
        '\u{06A4}' => Dual(['\u{FB6A}', '\u{FB6B}', '\u{FB6C}', '\u{FB6D}']),
        '\u{06A9}' => Dual(['\u{FB8E}', '\u{FB8F}', '\u{FB90}', '\u{FB91}']),
        '\u{06AF}' => Dual(['\u{FB92}', '\u{FB93}', '\u{FB94}', '\u{FB95}']),
        '\u{06CC}' => Dual(['\u{FBFC}', '\u{FBFD}', '\u{FBFE}', '\u{FBFF}']),
        _ => return None,
    };
    Some(j)
}

/// Shape logical-order `leads` into contextual Arabic presentation forms.
pub fn shape_leads(leads: &mut [char]) {
    let classes: Vec<Option<Joining>> = leads.iter().map(|&c| classify(c)).collect();
    if !classes.iter().any(Option::is_some) {
        return;
    }
    for i in 0..leads.len() {
        let Some(curr) = classes[i] else {
            continue;
        };
        let join_prev = curr.joins_right()
            && i > 0
            && classes[i - 1].is_some_and(Joining::joins_left);
        let join_next = curr.joins_left()
            && i + 1 < leads.len()
            && classes[i + 1].is_some_and(Joining::joins_right);
        if let Some(shaped) = curr.select(join_prev, join_next) {
            leads[i] = shaped;
        }
    }
}
