; Tree-sitter highlight query for markdown buffers.
;
; Registered automatically by plugin/define as markdown-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(atx_heading) @function
(setext_heading) @function
(fenced_code_block) @string
(indented_code_block) @string
