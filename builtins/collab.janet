# builtins/collab.janet — Collaborative editing via TCP (Sprint 13).
#
# Uses a simple text-based line protocol.  Each message is a single line whose
# first token (terminated by a space) is the message type:
#
#   SYNC <path> <base64-content>
#   CURSOR <client-id> <buffer-id> <offset>
#
# Janet owns all the protocol logic; Rust merely provides raw TCP send/receive.

(var *collab-server-id* nil)
(var *collab-conn-id* nil)
(var *collab-peer-cursors* @{})  # client-id string → {:buffer n :offset n}

# ── Simple text protocol helpers ──────────────────────────────────────────────

(defn- collab/encode-content
  "Encode content as base64 (poor man's: replace newlines with \x00 escape)."
  [s]
  # Replace \ with \\ first, then \n with \<n>
  (-> s
      (string/replace-all "\\" "\\\\")
      (string/replace-all "\n" "\\n")))

(defn- collab/decode-content
  "Decode content encoded by collab/encode-content."
  [s]
  # Two-pass decode: first unescape \\n → \n, then \\\\ → \
  (var out s)
  (set out (string/replace-all "\\n" "\n" out))
  (set out (string/replace-all "\\\\" "\\" out))
  out)

# ── Server mode ───────────────────────────────────────────────────────────────

(defn collab/start
  "Start a collaboration server on the given port."
  [port]
  (def sid (net/tcp-listen port))
  (set *collab-server-id* sid)
  (editor/log-message (string "Collab server started on port " port))

  # When a buffer is saved, sync its content to all clients
  (event/on "buffer-after-save"
    (fn [data]
      (when *collab-server-id*
        (def buf-id (get data :buffer-id 0))
        (def path (or (buffer/path buf-id) ""))
        (when (> (length path) 0)
          (def content (buffer/slice buf-id 0 (buffer/len buf-id)))
          (def msg (string "SYNC " path " " (collab/encode-content content)))
          (net/tcp-broadcast *collab-server-id* msg))))))

(defn collab/stop-server
  "Stop the collaboration server."
  []
  (when *collab-server-id*
    (net/tcp-stop *collab-server-id*)
    (set *collab-server-id* nil)
    (editor/log-message "Collab server stopped")))

# ── Client mode ───────────────────────────────────────────────────────────────

(defn collab/connect
  "Connect to a collaboration server at host:port."
  [host port]
  (def cid (net/tcp-connect host port))
  (set *collab-conn-id* cid)
  (editor/log-message (string "Connecting to collab server " host ":" port)))

(defn collab/disconnect
  "Disconnect from the collaboration server."
  []
  (when *collab-conn-id*
    (net/tcp-close *collab-conn-id*)
    (set *collab-conn-id* nil)
    (editor/log-message "Collab disconnected")))

# ── Incoming data from the server (client side) ───────────────────────────────

(event/on "tcp-data"
  (fn [data]
    (when (and *collab-conn-id*
               (= (get data :id "") (string *collab-conn-id*)))
      (def line (get data :data ""))
      (def sp (string/find " " line))
      (when sp
        (def msg-type (string/slice line 0 sp))
        (def rest (string/slice line (+ sp 1)))
        (case msg-type
          "SYNC"
          (do
            (def sp2 (string/find " " rest))
            (when sp2
              (def path (string/slice rest 0 sp2))
              (def encoded (string/slice rest (+ sp2 1)))
              (def content (collab/decode-content encoded))
              (def buf (buffer/get-by-name path))
              (when buf
                (buffer/set-read-only buf false)
                (def old-len (buffer/len buf))
                (when (> old-len 0) (buffer/delete buf 0 old-len))
                (buffer/insert buf 0 content)
                (buffer/set-read-only buf false)
                (editor/log-message (string "Collab sync: " path)))))
          "CURSOR"
          (do
            # Format: CURSOR <client-id> <buf-id> <offset>
            (def parts (string/split " " rest))
            (def client-id (get parts 0 ""))
            (def buf-id (scan-number (get parts 1 "0")))
            (def offset (scan-number (get parts 2 "0")))
            (put *collab-peer-cursors* client-id
                 {:buffer (or buf-id 0) :offset (or offset 0)})))))))

# ── Incoming data from clients (server side) ──────────────────────────────────

(event/on "tcp-client-data"
  (fn [data]
    (when *collab-server-id*
      (when (= (get data :server-id "") (string *collab-server-id*))
        (def client-id (get data :client-id ""))
        (def line (get data :data ""))
        (def sp (string/find " " line))
        (when sp
          (def msg-type (string/slice line 0 sp))
          (def rest (string/slice line (+ sp 1)))
          (case msg-type
            "CURSOR"
            (do
              # Broadcast cursor position to all other clients
              (def parts (string/split " " rest))
              (def buf-id (scan-number (get parts 0 "0")))
              (def offset (scan-number (get parts 1 "0")))
              (put *collab-peer-cursors* client-id
                   {:buffer (or buf-id 0) :offset (or offset 0)})
              (def broadcast-msg (string "CURSOR " client-id " " (or buf-id 0) " " (or offset 0)))
              (net/tcp-broadcast *collab-server-id* broadcast-msg))))))))

# ── Colon verbs ───────────────────────────────────────────────────────────────

(when (and (table? *colon-plugins*) (not (nil? (dyn '*colon-plugins*))))
  (put *colon-plugins* "collab-start"
    (fn [rest]
      (def port (or (scan-number (string/trim rest)) 7891))
      (collab/start port)))

  (put *colon-plugins* "collab-connect"
    (fn [rest]
      (def parts (string/split " " (string/trim rest)))
      (def host (or (get parts 0) "127.0.0.1"))
      (def port (or (scan-number (or (get parts 1) "7891")) 7891))
      (collab/connect host port)))

  (put *colon-plugins* "collab-stop"
    (fn [_]
      (collab/stop-server)
      (collab/disconnect))))
