# Magma scroll policy — configurable cursor-centering strategy.

(var *scroll-policy*  "standard")
(var *scroll-amount*  20)
(var *prev-scrolloff* nil)

(defn- visible-rows [sz]
  (let [h       (get sz :height 24)
        tab-row (if (ui/tab-bar-enabled) 1 0)]
    (- h 1 tab-row)))

(defn- compute-scrolloff [sz]
  (let [vis  (visible-rows sz)
        half (math/floor (/ vis 2))]
    (case *scroll-policy*
      "center"   half
      "adaptive" (let [buf   (buffer/current)
                       lines (if buf (buffer/line-count buf) 0)]
                   (if (> lines vis) half *scroll-amount*))
      *scroll-amount*)))

(defn apply-scroll-policy []
  (let [sz (surface/size)]
    (when sz
      (let [so (compute-scrolloff sz)]
        (when (not= so *prev-scrolloff*)
          (set *prev-scrolloff* so)
          (option/set "scrolloff" (string so)))))))

(when (and (table? *colon-plugins*) (not (nil? *colon-plugins*)))
  (put *colon-plugins* "scrollpolicy"
    (fn [args]
      (let [parts  (string/split " " (string/trim
                     (if (string? args) args (string/join args " "))))
            policy (get parts 0 "standard")
            amount (scan-number (get parts 1 "") 6)]
        (set *scroll-policy* policy)
        (when (> amount 0) (set *scroll-amount* amount))
        (set *prev-scrolloff* nil)
        (editor/log-message
          (string "Scroll policy: " policy
                  " (amount=" *scroll-amount* ")"))))))
