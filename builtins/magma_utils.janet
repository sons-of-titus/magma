# Magma utility functions under the magma/ namespace.
# Loaded immediately after init.janet so that all subsequent builtins can use them.

(def magma-api-version [0 24 0])

(defn magma/trim
  "Trim leading whitespace (spaces and tabs) from a string."
  [s]
  (var i 0)
  (def n (length s))
  (while (and (< i n) (or (= (string/slice s i (+ i 1)) " ") (= (string/slice s i (+ i 1)) "\t")))
    (set i (+ i 1)))
  (string/slice s i))

(defn magma/trim-trailing
  "Trim trailing whitespace (spaces and tabs) from a string."
  [s]
  (def n (length s))
  (var i (- n 1))
  (while (and (>= i 0) (or (= (string/slice s i (+ i 1)) " ") (= (string/slice s i (+ i 1)) "\t")))
    (set i (- i 1)))
  (string/slice s 0 (+ i 1)))

(defn magma/ensure-trailing-newline
  "Return the string with a single trailing newline appended if not present."
  [s]
  (def n (length s))
  (if (or (= n 0) (= (string/slice s (- n 1) n) "\n"))
    s
    (string s "\n")))

(defn magma/indent
  "Indent each line of `text` by `count` copies of `prefix`."
  [text count prefix]
  (def lines (string/split "\n" text))
  (def pad (string/repeat prefix count))
  (var result @[])
  (each line lines
    (array/push result (string pad line "\n")))
  (string/join result))

(defn magma/deep-merge
  "Deep-merge two associative data structures into a new table.
   Nested tables/structs are merged recursively; non-table values from `b` win over `a`."
  [a b]
  (var result (merge @{} a))
  (each [k v] (pairs b)
    (if (and (or (table? v) (struct? v))
             (or (table? (get result k)) (struct? (get result k))))
      (put result k (magma/deep-merge (get result k) v))
      (put result k v)))
  result)

# magma/time-now is provided as a C function registered in ecosystem_api.rs

(defn magma/debug-log
  "Print a debug message to stderr."
  [& args]
  (def msg (string/join args " "))
  (file/write stderr (string msg "\n")))

(defn magma/api-compat?
  "Return true if the running API version is at least MAJOR.MINOR.
   Calls (magma/api-compat? major minor)."
  [major minor]
  (or (> (magma-api-version 0) major)
      (and (= (magma-api-version 0) major)
           (>= (magma-api-version 1) minor))))
