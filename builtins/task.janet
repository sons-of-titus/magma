# Magma Task System — project build/test/run wiring.
#
# Listens for project-opened events to auto-define standard tasks, and
# registers the build/test/run commands the palette lists.

(defn- task-run-safe
  "Run a task by name, reporting any error instead of propagating it."
  [name]
  (try
    (do (task/run name) true)
    ([e] (editor/echo (string "task/" name " error: " e) "error") false)))

# --- Palette commands ----------------------------------------------------
# project/build|test|run dispatch through the Task System; fall back to
# task-run-safe if no project is active.

(command/define "build"
  (fn [&] (try (project/build) ([e] (task-run-safe "build")))))

(command/define "test"
  (fn [&] (try (project/test) ([e] (task-run-safe "test")))))

(command/define "run"
  (fn [&] (try (project/run) ([e] (task-run-safe "run")))))

# --- Task lifecycle event hooks ----------------------------------------

(event/on "task-started"
  (fn [data]
    (def name (get data :name "?"))
    (editor/echo (string "► " name))))

(event/on "task-completed"
  (fn [data]
    (def name (get data :name "?"))
    (def dur  (get data :duration-ms "?"))
    (editor/echo (string "✓ " name " (" dur "ms)"))))

(event/on "task-failed"
  (fn [data]
    (def name (get data :name "?"))
    (def err  (get data :error "?"))
    (editor/echo (string "✗ " name ": " err) "error")))

# --- Project integration -----------------------------------------------
# When a project opens, auto-define standard tasks from project markers.

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (when (> (length root) 0)
      (task/detect-project root))))
