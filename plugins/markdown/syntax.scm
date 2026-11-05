; Tree-sitter highlight query for markdown buffers.
;
; Registered automatically by plugin/define as markdown-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Structure ─────────────────────────────────────────────────────────────

(atx_heading) @function
(setext_heading) @function
(link_title) @string
(link_destination) @string
(link_label) @module

; ── Code ──────────────────────────────────────────────────────────────────

(fenced_code_block) @string
(indented_code_block) @string

; ── Emphasis ──────────────────────────────────────────────────────────────

; ── HTML within markdown ──────────────────────────────────────────────────

(html_block) @type
