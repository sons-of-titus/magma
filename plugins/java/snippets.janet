# Snippets for java — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "main" "public static void main(String[] args) {\n    ${0}\n}" "java" "main method")
(snippet/register "cls" "public class ${1:Name} {\n    ${0}\n}" "java" "class")
(snippet/register "fn" "${1:void} ${2:name}(${3:args}) {\n    ${0}\n}" "java" "method")
(snippet/register "sop" "System.out.println(${1:value});" "java" "println")
(snippet/register "fori" "for (int ${1:i} = 0; ${1:i} < ${2:n}; ${1:i}++) {\n    ${0}\n}" "java" "for loop")
(snippet/register "try" "try {\n    ${1}\n} catch (${2:Exception} ${3:e}) {\n    ${0}\n}" "java" "try/catch")
