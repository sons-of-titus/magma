# Markdown hover augmentation.  The kernel emits `hover-card` when the
# pointer-dwell card is created; this file attaches markdown-specific
# context — the target of the link whose label the pointer rests on, and
# the heading text when the hovered line is a heading.

(defn- link-target-for [word text]
  (var url nil)
  (each pat [(string "![" word "](") (string "[" word "](")]
    (when (and (nil? url) (string/find pat text))
      (def i (+ (string/find pat text) (length pat)))
      (when-let [close (string/find ")" text i)]
        (set url (string/slice text i close)))))
  url)

(defn- heading-text-for [word text]
  (def t (string/triml text))
  (when (and (string/has-prefix? "#" t) (string/find word t))
    (var i 0)
    (while (and (< i (length t)) (= "#" (string/slice t i (+ i 1))))
      (++ i))
    (string/trim (string/slice t i))))

(event/on "hover-card"
  (fn [ev]
    (def word (get ev :word))
    (def text (get ev :line-text))
    (when (and word text (not (empty? word)) (not (empty? text)))
      (when-let [url (link-target-for word text)]
        (hover/attach word "code" (string "→ " url)))
      (when-let [heading (heading-text-for word text)]
        (when (not (empty? heading))
          (hover/attach word "heading" heading))))))
