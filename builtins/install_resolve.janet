# Server-binary resolution for the LSP attach layer.
#
# One priority chain decides what command serves a language: the
# `<lang>-lsp-command` option (a broken override never silently falls
# back to a different server than the user named), a server the plugin
# bundles (:command, plugin-relative, platform-suffixed), one a previous
# install landed, a system install found beyond $PATH, and finally the
# manifest's :install pin — which asks the user (install/offer-fetch)
# rather than downloading uninvited.  Loads after install.janet, before
# lsp_attach.janet.

(var *missing-reported* @{})

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
  "Ask the user to install the manifest-pinned payload when nothing is
   installed yet; the server starts when they accept and install-completed
   arrives.  With offers off there is nothing to wait for — say so
   instead of going silent.  Never returns a command; the offer is the
   effect."
  [lang spec]
  (when-let [ispec (get spec :install)]
    (def bin (or (get ispec :bin) lang))
    (if (install/offers?)
      (install/offer-fetch "servers" lang bin ispec)
      (report-missing lang bin))
    nil))

(defn install/resolve-lsp-command
  "The command for `lang`'s language server per the priority chain in the
   header, or nil when nothing resolves (and the download has been offered
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
