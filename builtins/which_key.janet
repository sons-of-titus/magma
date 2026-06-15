# Which-key popup: shows available key bindings for a prefix.

# Show all bindings whose key starts with `prefix` in a *which-key* buffer.
(defn which-key/show [prefix]
  (def buf (buffer/find-or-create "*which-key*"))
  (buffer/set-ephemeral buf true)
  (buffer/set-read-only buf false)
  # Clear and rebuild
  (buffer/delete buf 0 (buffer/len buf))
  # Collect matching bindings across all active layers
  (def layers (keymap/list-layers))
  (def seen (table/new 16))
  (var count 0)
  (each layer layers
    (def bindings (keymap/list-layer layer))
    (each pair bindings
      (def k (pair 0))
      (def cmd (pair 1))
      (when (and (string/has-prefix? prefix k)
                 (not (= k prefix))
                 (nil? (seen k)))
        (put seen k true)
        (def suffix (string/slice k (length prefix)))
        (def line (string "  " suffix "\t→\t" cmd "\n"))
        (buffer/insert buf (buffer/len buf) line)
        (set count (+ count 1)))))
  (if (= count 0)
    (buffer/insert buf 0 (string "  (no bindings for prefix: " prefix ")\n")))
  (buffer/set-read-only buf true)
  # Open in a split below
  (editor/run-command "split-window-below")
  (editor/run-command "switch-to-buffer" (buffer/name buf)))

(command/define "which-key"
  (fn [& args]
    (def prefix (if (> (length args) 0) (args 0) ""))
    (which-key/show prefix)))

# Show which-key for the current pending prefix (removed pending-prefix C API)
