# Snippets for go — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fnc" "func ${1:name}(${2:args}) ${3:error} {\n	${0}\n}" "go" "function")
(snippet/register "main" "func main() {\n	${0}\n}" "go" "main")
(snippet/register "tst" "func Test${1:Name}(t *testing.T) {\n	${0}\n}" "go" "test")
(snippet/register "err" "if err != nil {\n	return ${1:nil}, err\n}\n${0}" "go" "error check")
(snippet/register "str" "type ${1:Name} struct {\n	${0}\n}" "go" "struct")
(snippet/register "itr" "for ${1:i}, ${2:v} := range ${3:xs} {\n	${0}\n}" "go" "range loop")
(snippet/register "iferr" "if ${1:err} != nil {\n	${0}\n}" "go" "if err")
