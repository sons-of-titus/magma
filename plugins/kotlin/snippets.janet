# Snippets for kotlin — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fun" "fun ${1:name}(${2:args}): ${3:Unit} {\n    ${0}\n}" "kotlin" "function")
(snippet/register "main" "fun main() {\n    ${0}\n}" "kotlin" "main")
(snippet/register "cls" "class ${1:Name}(${2:val dep: String}) {\n    ${0}\n}" "kotlin" "class")
(snippet/register "dcls" "data class ${1:Name}(\n    val ${2:name}: ${3:String}\n)" "kotlin" "data class")
(snippet/register "nul" "if (${1:x} == null) return ${2:null}\n${0}" "kotlin" "null guard")
(snippet/register "itr" "for (${1:item} in ${2:items}) {\n    ${0}\n}" "kotlin" "for loop")
