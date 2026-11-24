# Snippets for rust — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fn" "fn ${1:name}(${2:args}) {\n    ${0}\n}" "rust" "function")
(snippet/register "pfn" "pub fn ${1:name}(${2:args}) {\n    ${0}\n}" "rust" "public function")
(snippet/register "tst" "#[test]\nfn ${1:name}() {\n    ${0}\n}" "rust" "test")
(snippet/register "str" "struct ${1:Name} {\n    ${0}\n}" "rust" "struct")
(snippet/register "enm" "enum ${1:Name} {\n    ${0}\n}" "rust" "enum")
(snippet/register "imp" "impl ${1:Name} {\n    ${0}\n}" "rust" "impl block")
(snippet/register "mch" "match ${1:expr} {\n    ${2:pattern} => ${0},\n}" "rust" "match")
(snippet/register "itr" "for ${1:item} in ${2:iter} {\n    ${0}\n}" "rust" "for-in loop")
