# builtins/ssh_fs.janet — SSH/remote filesystem via process/spawn (Sprint 13).
#
# Provides a virtual filesystem abstraction for remote files reachable via SSH.
# All network I/O is delegated to the `ssh` binary already on the user's PATH.
# Janet owns the protocol; Rust provides process/spawn and buffer management.

(var *ssh-fs-pending* @{})  # proc-id (string) → callback fn

# ── Core read/write primitives ────────────────────────────────────────────────

(defn ssh-fs/read
  "Read a file from a remote host via SSH.
   `callback` is called with the file content as a string when done."
  [host path callback]
  (def pid (process/spawn "ssh" (string host " cat " path)))
  (def lines @[])
  (event/on "process-output"
    (fn [data]
      (when (= (get data :id "0") (string pid))
        (array/push lines (get data :line "")))))
  (event/on "process-exit"
    (fn [data]
      (when (= (get data :id "0") (string pid))
        (callback (string/join lines "\n"))))))

(defn ssh-fs/open
  "Open a remote file in a new buffer.
   The buffer is named `host:path` and its path is set to `ssh://host/path`."
  [host path]
  (def buf-name (string host ":" path))
  (ssh-fs/read host path
    (fn [content]
      (def buf (buffer/find-or-create buf-name))
      (buffer/set-read-only buf false)
      (def old-len (buffer/len buf))
      (when (> old-len 0) (buffer/delete buf 0 old-len))
      (buffer/insert buf 0 content)
      (buffer/set-read-only buf false)
      (buffer/set-path buf (string "ssh://" host path))
      (when-let [win (window/current)]
        (window/set-buffer win buf)))))

(defn ssh-fs/write
  "Write content to a remote file via SSH (uses `cat >`)."
  [host path content]
  (process/spawn "ssh" (string host " sh -c 'cat > " path "'")))

# ── Virtual filesystem router ─────────────────────────────────────────────────

(defn vfs/open
  "Open a local or remote file.  Supports `ssh://host/path` URIs."
  [uri]
  (if (string/has-prefix? "ssh://" uri)
    (do
      (def without-prefix (string/slice uri 6))
      (def slash-pos (string/find "/" without-prefix))
      (if slash-pos
        (do
          (def host (string/slice without-prefix 0 slash-pos))
          (def path (string/slice without-prefix slash-pos))
          (ssh-fs/open host path))
        (editor/log-message (string "vfs/open: invalid SSH URI: " uri))))
    (editor/run-command "open-file" uri)))

# ── Colon verbs ───────────────────────────────────────────────────────────────

(when (and (table? *colon-plugins*) (not (nil? (dyn '*colon-plugins*))))
  (put *colon-plugins* "ssh"
    (fn [rest]
      (def parts (string/split " " (string/trim rest)))
      (def host (get parts 0))
      (def path (or (get parts 1) "~"))
      (when (and host (> (length host) 0))
        (ssh-fs/open host path))))

  (put *colon-plugins* "vfs-open"
    (fn [rest]
      (vfs/open (string/trim rest)))))
