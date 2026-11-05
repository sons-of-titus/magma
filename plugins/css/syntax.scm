; Tree-sitter highlight query for css buffers.
;
; Registered automatically by plugin/define as css-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Selectors ─────────────────────────────────────────────────────────────

(tag_name) @type
(class_selector (class_name) @module)
(id_selector (id_name) @module)
(pseudo_class_selector (class_name) @keyword)
(pseudo_element_selector (tag_name) @keyword)
(attribute_selector (attribute_name) @property)
(namespace_name) @module

; ── Properties / values ───────────────────────────────────────────────────

(property_name) @property
(feature_name) @property
(plain_value) @variable
(color_value) @constant
(integer_value) @constant
(float_value) @constant
(unit) @keyword

; ── Strings ───────────────────────────────────────────────────────────────

(string_value) @string

; ── At-rules ──────────────────────────────────────────────────────────────

(at_keyword) @keyword
(import_statement (call_expression (function_name) @function))
(keyframes_name) @module

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
