# Snippets for cpp — LSP tab-stop syntax, registered language-scoped.
# Accepting a completion or (snippet-expand) inserts the body with live
# tab stops ($1 cycles, $0 is the final cursor position).


(snippet/register "fn" "${1:int} ${2:name}(${3:args}) {\n    ${0}\n}" "cpp" "function")
(snippet/register "mfn" "${1:Type} ${2:Class}::${3:name}(${4:args}) {\n    ${0}\n}" "cpp" "member function")
(snippet/register "cls" "class ${1:Name} {\npublic:\n    ${1:Name}();\n    ~${1:Name}() = default;\n\nprivate:\n    ${0}\n};" "cpp" "class")
(snippet/register "fori" "for (auto ${1:i} = 0u; ${1:i} < ${2:n}; ++${1:i}) {\n    ${0}\n}" "cpp" "for loop")
(snippet/register "forr" "for (const auto& ${1:item} : ${2:container}) {\n    ${0}\n}" "cpp" "range-for")
(snippet/register "ns" "namespace ${1:name} {\n${0}\n}  // namespace ${1:name}" "cpp" "namespace")
