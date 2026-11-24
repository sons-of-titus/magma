# Picker sources — search surfaces plugins register into the picker.
#
# The frontend emits `picker-source-query` on open and on every keystroke
# while a source picker is up; collectors here answer with
# (picker/push-items ...).  Entries ride the same payload conventions as
# the built-in kinds: "path<US>line" opens the file at the line.

(picker/register-source "document-symbols" "Go to Symbol in File" "Type a symbol name")
(picker/register-source "tags" "Go to Tag" "Type a tag name")

(defn- query-matches? [query name]
  (or (= (length query) 0)
      (string/has-prefix? query (string/ascii-lower name))))

(event/on "picker-source-query"
  (fn [ev]
    (def source (get ev :source))
    (def query (string/ascii-lower (string/trim (or (get ev :query) ""))))
    (cond
      (= source "document-symbols")
      (do
        (def buf-id (buffer/current))
        (if (nil? buf-id)
          (picker/push-items @[])
          (do
            (def path (or (buffer/path buf-id) ""))
            (def out @[])
            (each sym (semantic/symbols buf-id)
              (def name (get sym 0))
              (when (query-matches? query name)
                (def line (+ 1 (scan-number (get sym 3) 0)))
                (array/push out
                            {:title name
                             :detail (string (get sym 1))
                             :meta (string line)
                             :payload (string path "\x1f" line)})))
            (picker/push-items out))))

      (= source "tags")
      (do
        (def tbl (load-tags))
        (if (nil? tbl)
          (picker/push-items @[])
          (do
            (def out @[])
            (eachp [name entries] tbl
              (when (query-matches? query name)
                (each e entries
                  (array/push out
                              {:title name
                               :detail (string (get e :kind))
                               :meta (string (get e :file))
                               :payload (string (get e :file) "\x1f" (get e :line))}))))
            (picker/push-items out)))))))

(command/define "goto-symbol-in-file"
  (fn [&opt arg]
    (editor/run-command "gui-picker" "document-symbols")))

(keymap/set "ctrl-f12" "goto-symbol-in-file" :editor)
