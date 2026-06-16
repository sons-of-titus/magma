# Magma Task System — project build/test/run wiring (Phase 4)
#
# Listens for project-opened events to auto-define standard tasks,
# and registers colon-mode verbs :build :test :run for quick access.

(defn- task-run-safe
  "Run a task by name, printing any error instead of propagating it."
  [name]
  (try
    (do (task/run name) true)
    ([e] (print "task/" name " error: " e) false)))

# --- Colon verbs -------------------------------------------------------

(defn colon-build
  [_]
  (task-run-safe "build"))

(put *colon-plugins* "build" colon-build)

(defn colon-test-run
  [_]
  (task-run-safe "test"))

(put *colon-plugins* "test" colon-test-run)

(defn colon-run-project
  [_]
  (task-run-safe "run"))

(put *colon-plugins* "run" colon-run-project)

# --- Task lifecycle event hooks ----------------------------------------

(event/on "task-started"
  (fn [data]
    (def name (get data :name "?"))
    (print "► " name)))

(event/on "task-completed"
  (fn [data]
    (def name (get data :name "?"))
    (def dur  (get data :duration-ms "?"))
    (print "✓ " name " (" dur "ms)")))

(event/on "task-failed"
  (fn [data]
    (def name (get data :name "?"))
    (def err  (get data :error "?"))
    (print "✗ " name ": " err)))

# --- Project integration -----------------------------------------------
# When a project opens, auto-define standard tasks from project markers.

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (when (> (length root) 0)
      (task/detect-project root))))
