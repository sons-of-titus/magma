; Tree-sitter highlight query for html buffers.
;
; Registered automatically by plugin/define as html-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Tags ──────────────────────────────────────────────────────────────────

(tag_name) @type
(doctype) @keyword

; ── Attributes ────────────────────────────────────────────────────────────

(attribute_name) @property
(quoted_attribute_value) @string
(attribute_value) @string

; ── Scripts / styles ──────────────────────────────────────────────────────

(script_element) @string
(style_element) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
