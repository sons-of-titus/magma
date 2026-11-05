; Tree-sitter highlight query for json buffers.
;
; Registered automatically by plugin/define as json-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Structure ─────────────────────────────────────────────────────────────

(pair key: (string) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
(true) @constant
(false) @constant
(null) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
