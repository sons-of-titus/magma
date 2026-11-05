; Tree-sitter highlight query for toml buffers.
;
; Registered automatically by plugin/define as toml-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Structure ─────────────────────────────────────────────────────────────

(table (bare_key) @module)
(table_array_element (bare_key) @module)
(pair (bare_key) @property)
(pair (quoted_key) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
(boolean) @constant
(offset_date_time) @constant
(local_date_time) @constant
(local_date) @constant
(local_time) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
