#!/usr/bin/env bb
; Compact coverage for the Clojure TextMate grammar: λ, 東京, 🚀, 𝌆.
(ns fixture.basic
  "Small, readable syntax-highlighting examples."
  (:require [clojure.string :as str]))

(def ^:dynamic *greeting* "Hello")
(def unicode-data {:letter 'λ, :city "東京", :symbols #{"🚀" "𝌆"}})

(defn ^String greet [^{:tag String} name]
  (let [message (str *greeting* ", " name "!")]
    (str/upper-case message)))

(def sample
  {:nil nil, :flags [true false]
   :numbers [42 -7 3/4 0x2A 2r1010 6.02e23]
   :chars [\newline \λ]
   :qualified 'clojure.core/map})

(def banner
  "Unicode crosses lines:
東京 λ 🚀 𝌆")

(def word-pattern #"(?iu)\b(?:hello|東京|λ)\b")
(def quoted-form '(map inc [1 2 3]))
(def syntax-form `(println ~*greeting* ~@(:flags sample)))

(comment (greet "Ada") #'greet #_(discarded form))
