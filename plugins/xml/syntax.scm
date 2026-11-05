; Tree-sitter highlight query for xml buffers.
;
; Registered automatically by plugin/define as xml-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Tags ──────────────────────────────────────────────────────────────────

(STag (Name) @type)
(ETag (Name) @type)
(Attribute (Name) @property)
(AttValue) @string
(prolog) @keyword

; ── CDATA / entities ──────────────────────────────────────────────────────

(CData) @string
(EntityRef) @constant

; ── Comments ──────────────────────────────────────────────────────────────

(Comment) @comment
