# Snippets for typescript — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fn" "function ${1:name}(${2:args}): ${3:void} {\n    ${0}\n}" "typescript" "function")
(snippet/register "afn" "const ${1:name} = (${2:args}): ${3:void} => {\n    ${0}\n};" "typescript" "arrow function")
(snippet/register "cls" "class ${1:Name} {\n    constructor(private ${2:dep}: ${3:Dep}) {}\n    ${0}\n}" "typescript" "class")
(snippet/register "int" "interface ${1:Name} {\n    ${2:field}: ${3:string};\n}" "typescript" "interface")
(snippet/register "typ" "type ${1:Name} = {\n    ${2:field}: ${3:string};\n};" "typescript" "type alias")
(snippet/register "forof" "for (const ${1:item} of ${2:iter}) {\n    ${0}\n}" "typescript" "for-of loop")
(snippet/register "tryc" "try {\n    ${1}\n} catch (${2:e}) {\n    ${0}\n}" "typescript" "try/catch")
(snippet/register "log" "console.log(${1:value});" "typescript" "console.log")
