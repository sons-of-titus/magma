# Snippets for javascript — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fn" "function ${1:name}(${2:args}) {\n    ${0}\n}" "javascript" "function")
(snippet/register "afn" "const ${1:name} = (${2:args}) => {\n    ${0}\n};" "javascript" "arrow function")
(snippet/register "cls" "class ${1:Name} {\n    constructor(${2:args}) {\n        ${0}\n    }\n}" "javascript" "class")
(snippet/register "forof" "for (const ${1:item} of ${2:iter}) {\n    ${0}\n}" "javascript" "for-of loop")
(snippet/register "tryc" "try {\n    ${1}\n} catch (${2:e}) {\n    ${0}\n}" "javascript" "try/catch")
(snippet/register "log" "console.log(${1:value});" "javascript" "console.log")
(snippet/register "imp" "import ${1:name} from \"${2:module}\";" "javascript" "import")
(snippet/register "exp" "export ${1:const name = value};" "javascript" "export")
