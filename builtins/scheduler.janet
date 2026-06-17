# Magma Work Scheduler — Phase 9 Concurrency Model
#
# Event handlers for work progress / completion / failure, and colon-mode
# verbs for inspecting and cancelling background work.

# --- Event handlers ----------------------------------------------------

(event/on "scheduler-work-progress"
  (fn [data]
    (def id    (get data :id "?"))
    (def name  (get data :name "?"))
    (def done  (get data :done "?"))
    (def total (get data :total "?"))
    (when (> (length name) 0)
      (print "⟳ [" id "] " name " (" done "/" total ")"))))

(event/on "scheduler-work-completed"
  (fn [data]
    (def id   (get data :id "?"))
    (def name (get data :name "?"))
    (print "✓ [" id "] " name " done")))

(event/on "scheduler-work-failed"
  (fn [data]
    (def id    (get data :id "?"))
    (def name  (get data :name "?"))
    (def error (get data :error "?"))
    (print "✗ [" id "] " name ": " error)))

# --- Colon verbs --------------------------------------------------------

(defn colon-sched-list
  "List all work items tracked by the scheduler."
  [_]
  (def items (scheduler/list))
  (if (= (length items) 0)
    (print "No work items")
    (each item items
      (def id     (get item :id 0))
      (def name   (get item :name "?"))
      (def status (get item :status :unknown))
      (def done   (get item :done 0))
      (def total  (get item :total 0))
      (print "[" id "] " name " — " status " (" done "/" total ")"))))

(put *colon-plugins* "sched-list" colon-sched-list)

(defn colon-sched-cancel
  "Cancel a work item by id: :sched-cancel <id>"
  [arg]
  (def id (scan-number (string/trim (string arg))))
  (if (nil? id)
    (print "sched-cancel: provide a numeric work-id")
    (do
      (scheduler/cancel id)
      (print "Cancelled work item " id))))

(put *colon-plugins* "sched-cancel" colon-sched-cancel)
