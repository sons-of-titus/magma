# Snippets for python — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "dfn" "def ${1:name}(${2:args}):\n    ${0}" "python" "function")
(snippet/register "dcls" "def ${1:name}(${2:args}):\n    \"\"\"${3:docstring}\"\"\"\n    ${0}" "python" "function with docstring")
(snippet/register "cls" "class ${1:Name}:\n    def __init__(self${2:, args}):\n        ${0}" "python" "class")
(snippet/register "main" "def main() -> None:\n    ${0}\n\n\nif __name__ == \"__main__\":\n    main()" "python" "entry point")
(snippet/register "ifm" "if __name__ == \"__main__\":\n    ${0}" "python" "main guard")
(snippet/register "try" "try:\n    ${1}\nexcept ${2:Exception} as ${3:e}:\n    ${0}" "python" "try/except")
(snippet/register "forr" "for ${1:item} in ${2:iter}:\n    ${0}" "python" "for loop")
