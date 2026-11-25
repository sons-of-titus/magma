# Managed downloads — acquisition of servers and toolchains.
#
# Language plugins pin server payloads in their manifests
# (:lsp {"toml" {:install {:url :sha256 :format :bin}}}); toolchain specs
# carry the same :install.  One download per kind/id per session — the
# in-flight table guards double starts; completion and failure clear the
# status segment and post a notification.  Subscribers (lsp_attach,
# project detection) use install-completed to retry what was missing.
#
# How a server binary gets chosen lives in install_resolve.janet (the
# priority chain: option override, bundled :command, a previous install,
# a system install, the manifest's pinned download — which installs only
# after the user accepts the offer, servers-offer-install).

(var *in-flight* @{})

(var *missing-reported* @{})

(defn- file?
  "Does `path` exist as a regular file (or symlink to one)?"
  [path]
  (if-let [st (try (os/stat path) ([_] nil))]
    (or (= (get st :mode) :file) (= (get st :mode) :link))
    false))

(defn install/os-tag
  "The platform tag for bundled binaries: macos, linux, or windows."
  []
  (case (os/which) :macos "macos" :windows "windows" "linux"))

(defn install/offers?
  "Does the editor ask before installing a missing server or toolchain?
   The servers-offer-install option defaults on; off means never prompt
   and never download."
  []
  (not (= (option/get "servers-offer-install") "false")))

(defn install/in-flight? [key] (truthy? (get *in-flight* key)))

(defn install/bundled
  "Probe a plugin-shipped executable under plugin `root`: `<rel>` as the
   manifest wrote it, then with the platform suffix (.macos/.linux/.exe).
   Returns the existing path, or nil."
  [root rel]
  (when (and root (> (length root) 0) rel (> (length rel) 0))
    (def base (string root "/" rel))
    (def suffix (case (install/os-tag) "windows" "exe" (install/os-tag)))
    (cond
      (file? base) base
      (file? (string base "." suffix)) (string base "." suffix)
      nil)))

(defn install/fetch!
  "Start one managed download for `kind`/`id` unless one is already
   running.  `spec` is the manifest pin {:url :sha256 :format :bin}.
   Returns true when a fetch was actually started."
  [kind id name spec]
  (def key (string kind "/" id))
  (unless (get *in-flight* key)
    (put *in-flight* key true)
    (install/fetch name kind id spec)
    (status/set-segment "install" (string "⬇ " name))
    (notify/post "info"
                 (string "Downloading " name)
                 "Needed by this project — the download is checksum-pinned by the manifest.")
    true))

(defn- clear-flight [key]
  (put *in-flight* key nil)
  (status/set-segment "install" ""))

# ── Offers ───────────────────────────────────────────────────────────────────
# Nothing installs without the user's yes.  A pending offer carries the
# spec it would fetch; decline is remembered so the same question never
# re-asks this session.

(var *offered* @{})

(var *plugin-resolver* nil)

(defn install/set-plugin-resolver
  "plugin_install registers here: called as (f plugin-name accepted?) when
   a plugin offer is resolved."
  [f]
  (set *plugin-resolver* f))

(defn install/offer-fetch
  "Ask the user to install `name` (a manifest-pinned payload) for
   kind/id.  `kind` is the download slot (\"servers\"/\"toolchains\");
   the offer category is its singular.  The download starts only when
   they accept."
  [kind id name spec]
  (def offer-kind (case kind "servers" "server" "toolchains" "toolchain" kind))
  (def key (string offer-kind "/" id))
  (unless (= (get *offered* key) :declined)
    (put *offered* key {:state :pending :spec spec :name name})
    (offer/post offer-kind id
                (string "Install " name "?")
                "This project needs it; the download is pinned by the plugin manifest's checksum.")))

(defn install/offer-enable
  "Ask the user to enable a discovered plugin."
  [id name body]
  (def key (string "plugin/" id))
  (unless (= (get *offered* key) :declined)
    (put *offered* key {:state :pending :name name})
    (offer/post "plugin" id (string "Enable " name "?") body)))

(command/define "offer-resolved"
  (fn [args]
    (def parts (string/split " " (string/trim (string args))))
    (def kind (get parts 0))
    (def id (get parts 1))
    (def yes (= (get parts 2) "yes"))
    # The frontend pops the offer before answering; pop defensively so a
    # direct command answer works without it too.
    (offer/take kind id)
    (def key (string kind "/" id))
    (def rec (get *offered* key))
    (cond
      (= kind "plugin")
      (do
        (when *plugin-resolver* (*plugin-resolver* id yes))
        (put *offered* key (if yes :accepted :declined)))
      yes
      (do
        (put *offered* key {:state :accepted
                            :spec (get rec :spec)
                            :name (get rec :name)})
        (when (and rec (= (get rec :state) :pending))
          # The offer kind is singular; the download slot is plural.
          (def slot (case kind "server" "servers" "toolchain" "toolchains" kind))
          (install/fetch! slot id (get rec :name) (get rec :spec))))
      (put *offered* key :declined))))

(event/on "install-completed"
  (fn [ev]
    (clear-flight (string (get ev :kind) "/" (get ev :id)))
    (notify/post "info" (string "Installed " (get ev :name)))))

(event/on "install-failed"
  (fn [ev]
    (clear-flight (string (get ev :kind) "/" (get ev :id)))
    (notify/post "error" (string "Install failed: " (get ev :name))
                 (or (get ev :error) ""))))
