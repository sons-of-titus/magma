; Tree-sitter highlight query for zig buffers.
;
; Registered automatically by plugin/define as zig-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(IDENTIFIER) @variable

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

(FnProto function: (IDENTIFIER) @function)
(FieldOrFnCall function_call: (IDENTIFIER) @function)
(FieldOrFnCall field_access: (IDENTIFIER) @function)
(BUILTINIDENTIFIER) @function

; ── Types ─────────────────────────────────────────────────────────────────

(ContainerDecl) @type

; ── Constants ─────────────────────────────────────────────────────────────

(INTEGER) @constant
(FLOAT) @constant
"true" @constant
"false" @constant
"null" @constant
"undefined" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(STRINGLITERALSINGLE) @string
(LINESTRING) @string

; ── Comments ──────────────────────────────────────────────────────────────

(line_comment) @comment
(doc_comment) @comment
(container_doc_comment) @comment
