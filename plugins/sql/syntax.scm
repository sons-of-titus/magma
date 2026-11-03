; Tree-sitter highlight query for sql buffers.
;
; Registered automatically by plugin/define as sql-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "ADD" "ALL" "ALTER" "AND" "AS" "ASC" "BETWEEN" "BY"
  "CASE" "CAST" "CHECK" "COLUMN" "CONSTRAINT" "CREATE"
  "CROSS" "DEFAULT" "DELETE" "DESC" "DISTINCT" "DROP"
  "ELSE" "END" "EXCEPT" "EXISTS" "FOREIGN" "FROM"
  "FULL" "GROUP" "HAVING" "IN" "INDEX" "INNER" "INSERT"
  "INTERSECT" "INTO" "IS" "JOIN" "KEY" "LEFT" "LIKE"
  "LIMIT" "NOT" "NULL" "OFFSET" "ON" "OR" "ORDER"
  "OUTER" "PRIMARY" "REFERENCES" "RIGHT" "SELECT" "SET"
  "TABLE" "THEN" "TRANSACTION" "UNION" "UNIQUE" "UPDATE"
  "VALUES" "VIEW" "WHEN" "WHERE" "WITH"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_call function: (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(type) @type

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
"TRUE" @constant
"FALSE" @constant
"NULL" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(quoted_identifier) @property

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
