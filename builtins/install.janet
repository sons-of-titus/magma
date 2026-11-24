# Managed downloads — acquisition of servers and toolchains.
#
# Language plugins pin server payloads in their manifests
# (:lsp {"toml" {:install {:url :sha256 :format :bin}}}); toolchain specs
# carry the same :install.  One download per kind/id per session — the
# in-flight table guards double starts; completion and failure clear the
# status segment and post a notification.  Subscribers (lsp_attach,
# project detection) use install-completed to retry what was missing.
#
# Server binaries resolve in priority order: the `<lang>-lsp-command`
# option (a broken override never silently falls back), a server the
# plugin bundles (:command, plugin-relative, platform-suffixed), one a
# previous install landed, a system install, and finally the pinned
# download itself when servers-auto-install is on.

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

(defn install/auto?
  "Is automatic download of missing servers and toolchains enabled?
   The servers-auto-install option defaults on."
  []
  (not (= (option/get "servers-auto-install") "false")))

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
                 "Needed by this project — installed automatically (servers-auto-install).")
    true))

(defn- clear-flight [key]
  (put *in-flight* key nil)
  (status/set-segment "install" ""))

# ── Server resolution ────────────────────────────────────────────────────────

(defn- report-missing
  "Say a server is missing, once per command — a raw spawn failure is the
   wrong way for a user to learn a server is missing."
  [lang cmd]
  (unless (get *missing-reported* cmd)
    (put *missing-reported* cmd true)
    (notify/post "error"
                 (string "Language server not found: " cmd)
                 (string "LSP features disabled for " lang
                         " — install it or set " lang "-lsp-command")))
  nil)

(defn- bin-name
  "The server's conventional executable name: the :install pin's :bin,
   else a bundled :command's basename."
  [spec]
  (or (get-in spec [:install :bin])
      (when-let [cmd (get spec :command)]
        (last (string/split "/" cmd)))))

(defn- bundled-server [spec]
  (install/bundled (get spec :plugin-root) (get spec :command)))

(defn- installed-server [lang spec]
  (when-let [bin (bin-name spec)]
    (install/find "servers" lang bin)))

(defn- system-server [spec]
  (when-let [bin (bin-name spec)]
    (toolchain/find-extended bin)))

(defn- download-server
  "The manifest-pinned download when nothing is installed yet: the server
   starts when install-completed arrives.  With auto-install off there is
   nothing to wait for — say so instead of going silent.  Never returns a
   command; the fetch is the effect."
  [lang spec]
  (when-let [ispec (get spec :install)]
    (def bin (or (get ispec :bin) lang))
    (if (install/auto?)
      (install/fetch! "servers" lang bin ispec)
      (report-missing lang bin))
    nil))

(defn install/resolve-lsp-command
  "The command for `lang`'s language server per the priority chain in the
   header, or nil when nothing resolves (and the download has been kicked
   or the absence reported)."
  [lang spec]
  (def opt (option/get (string lang "-lsp-command")))
  (cond
    (and opt (> (length opt) 0))
    (or (toolchain/find-extended opt) (report-missing lang opt))
    spec
    (or (bundled-server spec)
        (installed-server lang spec)
        (system-server spec)
        (download-server lang spec))
    nil))

(event/on "install-completed"
  (fn [ev]
    (clear-flight (string (get ev :kind) "/" (get ev :id)))
    (notify/post "info" (string "Installed " (get ev :name)))))

(event/on "install-failed"
  (fn [ev]
    (clear-flight (string (get ev :kind) "/" (get ev :id)))
    (notify/post "error" (string "Install failed: " (get ev :name))
                 (or (get ev :error) ""))))
