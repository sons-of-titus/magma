# Magma project API — Janet helpers for project management (Sprint 4)
# Loaded by the Rust bridge after all other builtins.

# --- Colon mode verbs ---

(defn colon-project
  [path]
  (when (> (length path) 0)
    (project/set-root (string/trim path))
    (print "Project root set to: " path)))

(put *colon-plugins* "project" colon-project)

(defn colon-project-list
  [_]
  (def projs (project/list))
  (print "Registered projects:")
  (each p projs
    (print "  " (get p :name "?") " → " (get p :root "?"))))

(put *colon-plugins* "project-list" colon-project-list)

(defn colon-projects
  [_]
  (colon-project-list nil))

(put *colon-plugins* "projects" colon-projects)

# --- Auto-detect project root from buffer path ---
# When a file is opened, walk up the directory tree looking for
# project markers and set the root automatically.

(defn- dirname
  "Return the parent directory of a file path."
  [path]
  (def parts (string/split "/" path))
  (def n (length parts))
  (if (= n 1) "." (string/join (slice parts 0 (- n 1)) "/")))

(defn- find-project-root
  "Walk up from `path` looking for a project marker file.
   Returns the directory containing the marker, or nil."
  [path]
  (var dir (dirname path))
  (def markers @[".git" "Cargo.toml" "package.json" "go.mod"
                  "Gemfile" "setup.py" "project.janet" "Makefile"
                  "Rakefile" "deps.edn" "project.clj" "stack.yaml"])
  (var result nil)
  (while (and (not= dir "") (not= dir "/"))
    (var found false)
    (each marker markers
      (when (project/path-exists? (string dir "/" marker))
        (set found true)))
    (when found
      (set result dir)
      (set dir ""))
    (set dir (dirname dir)))
  (when (= result "/") (set result nil))
  result)

(defn- detect-project-on-buffer-created
  [data]
  (def path (get data :path ""))
  (when (and (> (length path) 0) (nil? (project/root))
    (def root (find-project-root path))
    (when root
      (project/set-root root)
      (project/push-recent root)
      (def buf-key (get data :buffer-id))
      (when buf-key
        (project/buffer-set-project (scan-number buf-key) (project/name)))
      (print "Detected project: " (project/name) " at " root)))))

(event/on "buffer-created" detect-project-on-buffer-created)

# --- File index helper ---
# Subscribe to file-indexed events to keep the project state fresh.

(event/on "file-indexed"
  (fn [data]
    (def count (get data :count "0"))
    (def pname (get data :project-name ""))
    (print "Indexed " count " files for project: " pname)))

# --- Project detection via colon verb :detect ---

(defn colon-detect
  [_]
  (def buf (buffer/current))
  (when buf
    (def p (buffer/path buf))
    (when p
      (def root (find-project-root p))
      (if root
        (do
          (project/set-root root)
          (project/push-recent root)
          (print "Detected project: " (project/name) " at " root))
        (print "No project root found from " p)))))

(put *colon-plugins* "detect" colon-detect)

# --- Workspace detection ---
# Check for common monorepo workspace markers and populate members.

(defn- detect-workspace-members
  "Check for workspace markers in the project root and populate
   workspace members if found."
  [root]
  (def cargo-toml (string root "/Cargo.toml"))
  (def package-json (string root "/package.json"))
  (def pnpm-workspace (string root "/pnpm-workspace.yaml"))
  (var members @[])

  (when (project/path-exists? cargo-toml)
    (def content (project/fs-read cargo-toml))
    (when (and content (string/find "[workspace]" content))
      (def lines (string/split "\n" content))
      (var in-workspace false)
      (var in-members false)
      (each line lines
        (def trimmed (string/trim line))
        (when (= trimmed "[workspace]")
          (set in-workspace true))
        (when in-workspace
          (when (= trimmed "members = [")
            (set in-members true))
          (when in-members
            (def m (string/find "=" trimmed))
            (when m (set in-members false)))
          (when (and in-members (string/find "\"" trimmed))
            (def member-name (string/trim (string/replace-all "\"" "" (string/replace-all "," "" trimmed))))
            (def member-root (string root "/" member-name))
            (when (project/path-exists? member-root)
              (array/push members {:name member-name :root member-root})))))))

  (when (project/path-exists? package-json)
    (def content (project/fs-read package-json))
    (when content
      (def workspaces (string/find "\"workspaces\"" content))
      (when workspaces
        (each line (string/split "\n" content)
          (def trimmed (string/trim line))
          (when (and (string/find "\"" trimmed) (string/find "/" trimmed))
            (def member-name (string/replace-all "\"" "" (string/replace-all "," "" (string/trim trimmed))))
            (def member-root (string root "/" member-name))
            (when (project/path-exists? member-root)
              (array/push members {:name member-name :root member-root})))))))

  (when (project/path-exists? pnpm-workspace)
    (def content (project/fs-read pnpm-workspace))
    (when content
      (each line (string/split "\n" content)
        (def trimmed (string/trim line))
        (when (and (string/find "\"" trimmed) (not (string/find ":" trimmed)))
          (def member-name (string/replace-all "\"" "" (string/trim trimmed)))
          (def member-root (string root "/" member-name))
          (when (project/path-exists? member-root)
            (array/push members {:name member-name :root member-root}))))))

  (when (> (length members) 0)
    (project/set-workspace-members members)))

# Run workspace detection on project-opened
(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (when (> (length root) 0)
      (detect-workspace-members root)
      (def members (project/workspace-members))
      (when (> (length members) 0)
        (print "Detected " (length members) " workspace members")))))

# --- Setup ---
# Initialise recent projects from persistent store

(defn- load-recent-on-ready
  [_]
  (each p (project/recent)
    (project/push-recent p)))

(event/once "editor-ready" load-recent-on-ready)
