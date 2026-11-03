; Tree-sitter highlight query for xml buffers.
;
; Registered automatically by plugin/define as xml-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Tags ──────────────────────────────────────────────────────────────────

(element_name) @type
(attribute (attribute_name) @property)
(attribute_value) @string
(quoted_attribute_value) @string
(prolog (Name) @keyword)

; ── CDATA / entities ──────────────────────────────────────────────────────

(CData) @string
(EntityRef) @constant

; ── Comments ──────────────────────────────────────────────────────────────

(Comment) @comment
