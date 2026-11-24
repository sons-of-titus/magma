# Snippets for c — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fn" "int ${1:name}(${2:void}) {\n    ${0}\n    return 0;\n}" "c" "function")
(snippet/register "fori" "for (int ${1:i} = 0; ${1:i} < ${2:n}; ${1:i}++) {\n    ${0}\n}" "c" "for loop")
(snippet/register "whi" "while (${1:cond}) {\n    ${0}\n}" "c" "while loop")
(snippet/register "ifr" "if (${1:ptr} == NULL) {\n    return ${2:-1};\n}\n${0}" "c" "null check")
(snippet/register "inc" "#include <${1:stdio.h}>\n" "c" "include")
