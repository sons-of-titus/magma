# builtins/net.janet — HTTP and network helpers for Janet extensions (Sprint 13).
#
# Provides a callback-based API on top of the raw net/http-* C functions.
# All HTTP requests are fire-and-forget; the response arrives asynchronously
# via the http-response / http-error events.
#
# Janet API:
#   (net/on-response id callback)  — register one-shot callback for an HTTP id
#   :http <url>                    — colon verb: fetch URL, show in *net-output*

(var *net-pending* @{})  # request-id (string) → callback function

# ── Event handlers ────────────────────────────────────────────────────────────

(event/on "http-response"
  (fn [data]
    (def id (get data :id "0"))
    (def cb (get *net-pending* id))
    (when cb
      (put *net-pending* id nil)
      (cb {:status (get data :status "0") :body (get data :body "")}))))

(event/on "http-error"
  (fn [data]
    (def id (get data :id "0"))
    (def cb (get *net-pending* id))
    (when cb
      (put *net-pending* id nil)
      (cb {:error (get data :error "")}))))

# ── Public API ────────────────────────────────────────────────────────────────

(defn net/on-response
  "Register a one-shot callback for an HTTP request identified by id.
   The callback receives a table with :status + :body on success,
   or :error on failure."
  [id cb]
  (put *net-pending* (string id) cb))

# ── :http colon verb ──────────────────────────────────────────────────────────

(when (and (table? *colon-plugins*) (not (nil? (dyn '*colon-plugins*))))
  (put *colon-plugins* "http"
    (fn [rest]
      (def url (string/trim rest))
      (when (> (length url) 0)
        (def id (net/http-get url))
        (net/on-response id
          (fn [resp]
            (def buf (buffer/find-or-create "*net-output*"))
            (buffer/set-ephemeral buf true)
            (buffer/set-read-only buf false)
            (buffer/insert buf (buffer/len buf)
              (string "--- " url " [" (get resp :status "") "] ---\n"
                      (get resp :body "") "\n"))
            (buffer/set-read-only buf true)))))))
