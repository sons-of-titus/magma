; Tree-sitter highlight query for zig buffers.
;
; Registered automatically by plugin/define as zig-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "addrspace" "align" "allowzero" "and" "anyframe" "anytype"
  "asm" "async" "await" "break" "callconv" "catch" "comptime"
  "const" "continue" "defer" "else" "enum" "errdefer" "error"
  "export" "extern" "fn" "for" "if" "inline" "linksection"
  "noalias" "noinline" "nosuspend" "opaque" "or" "orelse"
  "packed" "pub" "resume" "return" "struct" "suspend" "switch"
  "test" "threadlocal" "try" "union" "unreachable" "usingnamespace"
  "var" "volatile" "while"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(fn_proto name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_access field: (identifier) @function))
(builtin_call_expression (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(struct_declaration) @type
(enum_declaration) @type
(union_declaration) @type

; ── Constants ─────────────────────────────────────────────────────────────

(integer_literal) @constant
(float_literal) @constant
"true" @constant
"false" @constant
"null" @constant
"undefined" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(multiline_string_literal) @string

; ── Comments ──────────────────────────────────────────────────────────────

(line_comment) @comment
(doc_comment) @comment
(container_doc_comment) @comment
