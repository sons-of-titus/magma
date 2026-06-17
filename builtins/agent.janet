# Agent system event handlers and colon verbs (Phase 11).
#
# Agents observe editor state, reason about a goal, and dispatch commands.
# They never block the UI — all work runs on background threads.
#
# Events:
#   agent-thought  {:session-id :thought}
#   agent-action   {:session-id :command :description}
#   agent-result   {:session-id :result}
#   agent-error    {:session-id :error}
#
# Colon verbs:
#   :agent-ask  <prompt>  — ask the agent a question
#   :agent-task <desc>    — submit a structured task

(event/on "agent-thought"
  (fn [data]
    (let [sid    (get data "session-id" "?")
          thought (get data "thought" "")]
      (editor/log-message (string "[agent:" sid "] " thought)))))

(event/on "agent-action"
  (fn [data]
    (let [sid  (get data "session-id" "?")
          cmd  (get data "command" "")
          desc (get data "description" "")]
      (editor/log-message (string "[agent:" sid "] action: " desc " (" cmd ")")))))

(event/on "agent-result"
  (fn [data]
    (let [sid    (get data "session-id" "?")
          result (get data "result" "")]
      (editor/log-message (string "[agent:" sid "] done: " result)))))

(event/on "agent-error"
  (fn [data]
    (let [sid (get data "session-id" "?")
          err (get data "error" "")]
      (editor/log-message (string "[agent:" sid "] error: " err)))))

# ── agent/on-event helper ─────────────────────────────────────────────────────

(defn agent/on-event
  "Subscribe `callback` to the agent event `kind`.
  Recognised kinds: thought | action | result | error."
  [kind callback]
  (event/on (string "agent-" kind) callback))

# ── Colon verbs ───────────────────────────────────────────────────────────────

(colon/define "agent-ask"
  (fn [prompt]
    (if (empty? prompt)
      (editor/log-message "Usage: :agent-ask <prompt>")
      (let [id (agent/ask prompt)]
        (editor/log-message (string "agent session " id " started"))))))

(colon/define "agent-task"
  (fn [desc]
    (if (empty? desc)
      (editor/log-message "Usage: :agent-task run:<name> | goto:<symbol> | diagnose:<path>")
      (let [id (agent/run-task desc)]
        (editor/log-message (string "agent task " id " submitted"))))))
