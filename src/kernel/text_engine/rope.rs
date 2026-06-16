//! Rope backed by `ropey::Rope`.
///
/// The public API is byte-offset based to match the rest of the buffer layer.
/// Internally all operations convert byte offsets to char indices before
/// calling into ropey.  Byte offsets that fall in the middle of a multi-byte
/// sequence are silently snapped to the nearest preceding char boundary.
use std::borrow::Cow;
use ropey::Rope as RopeyRope;

#[derive(Clone, Debug)]
pub struct Rope {
    inner: RopeyRope,
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

impl Rope {
    pub fn new() -> Self {
        Rope { inner: RopeyRope::new() }
    }

    pub fn from_string(s: &str) -> Self {
        Rope { inner: RopeyRope::from_str(s) }
    }

    pub fn len(&self) -> usize {
        self.inner.len_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.len_bytes() == 0
    }

    pub fn insert(&mut self, byte_offset: usize, text: &str) {
        if text.is_empty() { return; }
        let char_idx = self.byte_to_char_idx(byte_offset);
        self.inner.insert(char_idx, text);
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        if start >= end { return; }
        let end = end.min(self.inner.len_bytes());
        if start >= end { return; }
        let start_char = self.byte_to_char_idx(start);
        let end_char   = self.byte_to_char_idx(end);
        if start_char < end_char {
            self.inner.remove(start_char..end_char);
        }
    }

    pub fn slice(&self, start: usize, end: usize) -> Cow<'_, str> {
        if start >= end { return Cow::Borrowed(""); }
        let end = end.min(self.inner.len_bytes());
        if start >= end { return Cow::Borrowed(""); }
        let start_char = self.byte_to_char_idx(start);
        let end_char   = self.byte_to_char_idx(end);
        Cow::Owned(self.inner.slice(start_char..end_char).to_string())
    }

    pub fn char_at(&self, byte_offset: usize) -> Option<char> {
        if byte_offset >= self.inner.len_bytes() { return None; }
        let char_idx = self.byte_to_char_idx(byte_offset);
        if char_idx >= self.inner.len_chars() { return None; }
        Some(self.inner.char(char_idx))
    }

    pub fn line_count(&self) -> usize {
        self.inner.len_lines()
    }

    pub fn line(&self, n: usize) -> Option<Cow<'_, str>> {
        if n >= self.inner.len_lines() { return None; }
        let raw = self.inner.line(n).to_string();
        // ropey includes the line terminator; strip it.
        let trimmed = raw.strip_suffix('\n')
            .map(|s| s.strip_suffix('\r').unwrap_or(s))
            .unwrap_or(&raw)
            .to_string();
        Some(Cow::Owned(trimmed))
    }

    pub fn line_start_offset(&self, n: usize) -> Option<usize> {
        if n >= self.inner.len_lines() { return None; }
        let char_idx = self.inner.line_to_char(n);
        Some(self.inner.char_to_byte(char_idx))
    }

    pub fn all_lines(&self) -> Vec<String> {
        (0..self.inner.len_lines())
            .filter_map(|i| self.line(i).map(|l| l.into_owned()))
            .collect()
    }

    // ── Internal ──────────────────────────────────────────────────────────

    /// Convert a byte offset to a ropey char index.  If the byte offset is
    /// not on a UTF-8 char boundary (e.g. the caller passed a mid-sequence
    /// byte), it is snapped to the nearest preceding boundary.
    fn byte_to_char_idx(&self, byte_offset: usize) -> usize {
        let len_bytes = self.inner.len_bytes();
        if byte_offset == 0       { return 0; }
        if byte_offset >= len_bytes { return self.inner.len_chars(); }

        // chunks_at_byte returns the chunk that contains byte_offset together
        // with the byte and char indices of that chunk's start.
        let (mut chunks, chunk_byte_start, chunk_char_start, _) =
            self.inner.chunks_at_byte(byte_offset);

        if let Some(chunk) = chunks.next() {
            let local = byte_offset - chunk_byte_start;
            // Walk back from local to the nearest char boundary.
            let mut b = local.min(chunk.len());
            while b > 0 && !chunk.is_char_boundary(b) { b -= 1; }
            chunk_char_start + chunk[..b].chars().count()
        } else {
            self.inner.len_chars()
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rope() {
        let r = Rope::new();
        assert_eq!(r.len(), 0);
        assert!(r.is_empty());
        assert_eq!(r.line_count(), 1);
    }

    #[test]
    fn insert_and_slice() {
        let mut r = Rope::new();
        r.insert(0, "Hello, World!");
        assert_eq!(r.len(), 13);
        assert_eq!(&*r.slice(0, 5), "Hello");
        assert_eq!(&*r.slice(7, 12), "World");
    }

    #[test]
    fn delete() {
        let mut r = Rope::from_string("Hello, World!");
        r.delete(5, 7);
        assert_eq!(&*r.slice(0, r.len()), "HelloWorld!");
    }

    #[test]
    fn insert_at_offset() {
        let mut r = Rope::from_string("Heloworld");
        r.insert(3, "l");
        assert_eq!(&*r.slice(0, r.len()), "Helloworld");
    }

    #[test]
    fn lines() {
        let r = Rope::from_string("line1\nline2\nline3");
        assert_eq!(r.line_count(), 3);
        assert_eq!(r.line(0).as_deref(), Some("line1"));
        assert_eq!(r.line(1).as_deref(), Some("line2"));
        assert_eq!(r.line(2).as_deref(), Some("line3"));
        assert_eq!(r.line(3), None);
    }

    #[test]
    fn lines_trailing_newline() {
        let r = Rope::from_string("a\nb\n");
        assert_eq!(r.line_count(), 3); // "a", "b", ""
        assert_eq!(r.line(0).as_deref(), Some("a"));
        assert_eq!(r.line(1).as_deref(), Some("b"));
        assert_eq!(r.line(2).as_deref(), Some(""));
    }

    #[test]
    fn line_start_offset() {
        let r = Rope::from_string("abc\ndefg\nhi");
        assert_eq!(r.line_start_offset(0), Some(0));
        assert_eq!(r.line_start_offset(1), Some(4));  // after 'abc\n'
        assert_eq!(r.line_start_offset(2), Some(9));  // after 'defg\n'
        assert_eq!(r.line_start_offset(3), None);
    }

    #[test]
    fn char_at_ascii() {
        let r = Rope::from_string("hello");
        assert_eq!(r.char_at(0), Some('h'));
        assert_eq!(r.char_at(4), Some('o'));
        assert_eq!(r.char_at(5), None);
    }

    #[test]
    fn multibyte_insert_and_slice() {
        let mut r = Rope::new();
        r.insert(0, "héllo");  // 'é' = 2 bytes
        assert_eq!(r.len(), 6);
        assert_eq!(&*r.slice(0, r.len()), "héllo");
    }

    #[test]
    fn multibyte_delete() {
        let mut r = Rope::from_string("héllo");
        r.delete(1, 3);  // delete 'é'
        assert_eq!(&*r.slice(0, r.len()), "hllo");
    }

    #[test]
    fn char_at_multibyte() {
        let r = Rope::from_string("héllo");
        assert_eq!(r.char_at(0), Some('h'));
        assert_eq!(r.char_at(1), Some('é'));  // 'é' starts at byte 1
        assert_eq!(r.char_at(3), Some('l'));  // after 'é'
    }

    #[test]
    fn non_boundary_byte_snaps_to_boundary() {
        let r = Rope::from_string("héllo");
        // byte 2 is the second byte of 'é' (not a char boundary)
        // slice(2, 5) should snap 2 → 1 and give "éll"
        let s = r.slice(2, 5);
        assert!(s.starts_with('é') || s.starts_with('l'),
            "non-boundary slice must not panic and must return valid UTF-8, got {:?}", s);
    }

    #[test]
    fn emoji_does_not_panic() {
        let mut r = Rope::from_string("hi😀bye");
        // emoji is 4 bytes at offset 2; deleting mid-emoji must not panic
        r.delete(3, 5);
        let _ = r.slice(0, r.len()); // must not panic
    }

    #[test]
    fn large_insert_sequence() {
        let mut r = Rope::new();
        for i in 0..200u32 {
            let s = format!("line{}\n", i);
            let pos = r.len();
            r.insert(pos, &s);
        }
        assert_eq!(r.line_count(), 201); // 200 lines + final empty line
    }
}
