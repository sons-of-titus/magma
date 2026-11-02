//! Chunk-wise scanning over the rope — motions and search without
//! materializing document slices.  Every allocation here is bounded by
//! one rope chunk plus a needle-sized overlap, never by document size.

use super::Buffer;

impl Buffer {
    /// Byte offset of the first char at/after `from` matching `pred`.
    pub fn find_char_from(&self, from: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
        self.char_span(from, self.len())
            .find(|&(_, c)| pred(c))
            .map(|(off, _)| off)
    }

    /// Byte offset of the last char strictly before `before` matching
    /// `pred`.  Scans forward with bounded memory — no prefix copy.
    pub fn rfind_char_before(&self, before: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
        self.char_span(0, before.min(self.len()))
            .filter(|&(_, c)| pred(c))
            .last()
            .map(|(off, _)| off)
    }

    /// Byte offset of the last char strictly before `before` matching
    /// `pred`, scanning backwards: the cost is the distance to the match
    /// (geometrically growing windows), never the prefix length.  `before`
    /// must be a char boundary.
    pub fn rfind_char_backwards(&self, before: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
        let len = self.len();
        let before = before.min(len);
        if before == 0 {
            return None;
        }
        let mut window = 64usize;
        loop {
            let from = before.saturating_sub(window);
            let mut last: Option<usize> = None;
            for (off, c) in self.char_span(from, before) {
                if pred(c) {
                    last = Some(off);
                }
            }
            if last.is_some() || from == 0 {
                return last;
            }
            window *= 8;
        }
    }

    /// Byte offset of the char boundary immediately before `pos` — O(log n)
    /// through the rope, no scan.  The backspace-class primitive.
    pub fn prev_char_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            0
        } else {
            self.doc.snap_offset(pos - 1)
        }
    }

    /// Byte offset of the first occurrence of `needle` at/after `from`,
    /// across chunk boundaries.
    pub fn find_sub_from(&self, from: usize, needle: &str) -> Option<usize> {
        if needle.is_empty() {
            return None;
        }
        let start = from.min(self.len());
        let n = needle.len();
        let mut prev_end = start; // absolute offset up to which matches were checked
        let mut carry = String::new();
        let (chunks, chunk0) = self.rope_handle().chunks_from(start);
        let mut carry_base = chunk0;
        for chunk in chunks {
            let mut window = std::mem::take(&mut carry);
            window.push_str(chunk);
            let win_end = carry_base + window.len();
            let mut rel = 0usize;
            while let Some(r) = window[rel..].find(needle) {
                let abs = carry_base + rel + r;
                // Skip matches fully contained in the previous window —
                // they were already checked.  Matches extending past it
                // were impossible to see before this window.
                if abs >= start && abs + n > prev_end {
                    return Some(abs);
                }
                rel += r + 1; // +1: overlapping occurrences stay findable
            }
            prev_end = win_end;
            let keep = window.len().saturating_sub(n - 1);
            carry = window[keep..].to_string();
            carry_base += keep;
        }
        None
    }

    /// Byte offset of the last occurrence of `needle` starting at or
    /// after `from` and ending at or before `before`.  Bounded memory.
    pub fn rfind_sub_before(&self, before: usize, needle: &str) -> Option<usize> {
        if needle.is_empty() {
            return None;
        }
        let end = before.min(self.len());
        let n = needle.len();
        let mut last: Option<usize> = None;
        let mut carry = String::new();
        let (chunks, chunk0) = self.rope_handle().chunks_from(0);
        let mut carry_base = chunk0;
        for chunk in chunks {
            let mut window = std::mem::take(&mut carry);
            window.push_str(chunk);
            let win_end = carry_base + window.len();
            let mut rel = 0usize;
            while let Some(r) = window[rel..].find(needle) {
                let abs = carry_base + rel + r;
                if abs + n <= end {
                    last = Some(abs);
                } else if abs > end {
                    break;
                }
                rel += r + 1;
            }
            if win_end > end {
                break; // past the limit: no later match can qualify
            }
            let keep = window.len().saturating_sub(n - 1);
            carry = window[keep..].to_string();
            carry_base += keep;
        }
        last
    }

    /// Chunk-fed `(byte offset, char)` pairs over `[from, to)` — the
    /// substrate for depth-matching braces and word walks.
    pub fn char_span(&self, from: usize, to: usize) -> impl Iterator<Item = (usize, char)> + '_ {
        let end = to.min(self.len());
        let start = from.min(end);
        let (chunks, chunk0) = self.rope_handle().chunks_from(start);
        let mut base = chunk0;
        chunks
            .scan((), move |_, ch| {
                let b = base;
                base += ch.len();
                Some((b, ch))
            })
            .flat_map(|(b, ch)| ch.char_indices().map(move |(i, c)| (b + i, c)))
            .skip_while(move |(off, _)| *off < start)
            .take_while(move |(off, _)| *off < end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use id::BufferId;

    fn buf(text: &str) -> Buffer {
        Buffer::from_string(BufferId(1), "t", text)
    }

    #[test]
    fn char_predicates_scan_forward_and_back() {
        let b = buf("hello wörld");
        assert_eq!(b.find_char_from(0, |c| c == 'w'), Some(6));
        assert_eq!(
            b.find_char_from(7, |c| c == 'r'),
            Some(9),
            "ö is 2 bytes but 1 char"
        );
        assert_eq!(b.rfind_char_before(6, |c| c == 'l'), Some(3));
        assert_eq!(b.find_char_from(0, |c| c == 'z'), None);
    }

    #[test]
    fn substring_search_respects_offsets() {
        let b = buf("abc abc abc");
        assert_eq!(b.find_sub_from(0, "abc"), Some(0));
        assert_eq!(b.find_sub_from(1, "abc"), Some(4));
        assert_eq!(b.find_sub_from(5, "abc"), Some(8));
        assert_eq!(b.rfind_sub_before(11, "abc"), Some(8));
        assert_eq!(
            b.rfind_sub_before(8, "abc"),
            Some(4),
            "match must end before the limit"
        );
        assert_eq!(b.find_sub_from(0, "abd"), None);
    }

    #[test]
    fn overlapping_occurrences_all_findable() {
        let b = buf("aaaa");
        assert_eq!(b.find_sub_from(0, "aa"), Some(0));
        assert_eq!(b.find_sub_from(1, "aa"), Some(1));
        assert_eq!(b.find_sub_from(2, "aa"), Some(2));
        assert_eq!(b.rfind_sub_before(4, "aa"), Some(2));
    }

    #[test]
    fn substring_matches_span_chunk_boundaries() {
        // A document large enough to span many rope chunks, with needles
        // at regular intervals so some straddle boundaries whatever the
        // chunk layout is.
        let unit = "0123456789abcdef".repeat(8); // 128 bytes
        let mut text = String::new();
        for i in 0..3000 {
            text.push_str(&unit);
            text.push_str(&format!("n{i}x"));
        }
        let b = buf(&text);
        for i in 0..3000 {
            let pat = format!("n{i}x");
            let expect = text.find(&pat).unwrap();
            assert_eq!(b.find_sub_from(0, &pat), Some(expect), "pattern {i}");
        }
        let last_pat = "n2999x";
        let last_at = text.rfind(last_pat).unwrap();
        assert_eq!(b.rfind_sub_before(text.len(), last_pat), Some(last_at));
    }

    #[test]
    fn multibyte_needles_scan() {
        // Second "héllo" starts at byte 14 (each ö/é is two bytes).
        let b = buf("héllo wörld héllo");
        assert_eq!(b.find_sub_from(1, "héllo"), Some(14));
        assert_eq!(b.rfind_sub_before(20, "héllo"), Some(14));
        assert_eq!(
            b.rfind_sub_before(19, "héllo"),
            Some(0),
            "the second match ends past the limit; the prefix one still qualifies"
        );
    }

    #[test]
    fn char_span_iterates_offsets_and_chars() {
        let b = buf("aé\nb");
        let got: Vec<(usize, char)> = b.char_span(0, 4).collect();
        assert_eq!(got, vec![(0, 'a'), (1, 'é'), (3, '\n')]);
        let got: Vec<(usize, char)> = b.char_span(0, b.len()).collect();
        assert_eq!(got, vec![(0, 'a'), (1, 'é'), (3, '\n'), (4, 'b')]);
        let got: Vec<(usize, char)> = b.char_span(2, 4).collect();
        assert_eq!(got, vec![(3, '\n')], "span respects both ends");
    }
}
