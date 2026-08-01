#!/usr/bin/env clojure
; Broad grammar stress fixture. Every reader and collection form is closed.
; Unicode coverage includes BMP λ and 東京, plus astral 🚀 and 𝌆.
(ns ^{:doc "Reviewable TextMate stress data"
      :author "fixture"}
  fixture.stress
  (:refer-clojure :exclude [update])
  (:require [clojure.set :as set]
            [clojure.string :as str])
  (:import (java.time Instant)
           (java.util UUID)))

(declare render summarize)
(def ^:dynamic *locale* :en)
(def ^:private default-city "東京")
(def ^String rocket-label "λ-launch 🚀 𝌆")

(def truthy-values [nil true false])
(def integer-values
  [0 1 -1 +42 9223372036854775808N
   0755 0xCAFE 2r101010 36rZ])
(def decimal-values
  [3.14159 -0.25 +6.02e23 1E-9 12.50M])
(def exceptional-values [##Inf ##-Inf ##NaN])
(def ratios [1/2 -22/7 +355/113])
(def characters
  [\a \space \newline \tab \u03bb \λ \東])

(def keyword-values
  [:plain :kebab-case :question? :bang!
   :fixture.stress/qualified ::local ::alias-like])

(def symbol-values
  '[plain kebab-case predicate? mutation!
    clojure.core/map fixture.stress/render λ 東京])

(def launch-copy
  "Mission notes:
  λ is a BMP letter; 東京 is a BMP city.
  🚀 and 𝌆 are astral code points.
  Escapes remain visible: \"quoted\", \\slash, \tab, \n.")

(def multiline-pattern
  #"(?ms)^MISSION\s+λ
  .*東京.*
  .*🚀.*𝌆$")

(def token-pattern
  #"(?ix)
    (?:alpha|beta)       # words
    \s+
    [\p{L}\p{N}_-]+")

(def nested-data
  {:identity {:id 7
              :name "Ada"
              :active? true}
   :coordinates [35.6762 139.6503]
   :routes (list :ground :orbit :return)
   :tags #{:science :clojure :東京}
   :empty {:map {} :vector [] :list () :set #{}}
   :unicode {:lambda 'λ :city '東京 :flight "🚀" :tetragram "𝌆"}})

(def namespaced-maps
  [#:person{:first "Grace" :last "Hopper"}
   #::{:id 42 :state :ready}
   #::str{:first "Katherine" :last "Johnson"}])

(def temporal-values
  [#inst "2024-01-02T03:04:05.000-00:00"
   #uuid "123e4567-e89b-12d3-a456-426614174000"])

(def quoted-list
  '(alpha (beta gamma) [delta] {:epsilon zeta}))

(def syntax-quoted-list
  `(let [value# ~rocket-label]
     (println ~'value# ~@[:from default-city])))

(def quoted-symbol 'fixture.stress/render)
(def quoted-var #'render)
(def dereferenced-locale @#'*locale*)
(def discarded-example
  [1 #_(this form is intentionally discarded) 2])

(def increment-all #(map inc %))
(def pair->map #(hash-map :left %1 :right %2))
(def rest-counter #(count %&))

(defprotocol Renderable
  "Values that can produce reviewable text."
  (render [value] [value options])
  (dimensions [value]))

(defrecord Launch [id city payload]
  Renderable
  (render [this]
    (render this {:locale *locale*}))
  (render [{:keys [id city payload]} options]
    (format "%s:%s:%s:%s"
            (name (:locale options)) id city payload))
  (dimensions [_]
    {:fields 3 :units :record}))

(defmacro when-let+
  "Like when-let, with a visible macro expansion."
  [bindings & body]
  `(when-let [value# ~bindings]
     (let [result# (do ~@body)]
       {:value value# :result result#})))

(defmacro with-locale [locale & body]
  `(binding [*locale* ~locale]
     ~@body))

(defn normalize-name
  ^String [value]
  (-> value
      str
      str/trim
      str/lower-case
      (str/replace #"\s+" "-")))

(defn route-summary [routes]
  (->> routes
       (filter keyword?)
       (map name)
       (interpose " → ")
       (apply str)))

(defn enrich [launch]
  (cond-> launch
    (nil? (:city launch)) (assoc :city default-city)
    (:payload launch) (clojure.core/update :payload str)
    true (assoc :checked? true)))

(defn classify-number [number]
  (cond
    (neg? number) :negative
    (zero? number) :zero
    (even? number) :positive-even
    :else :positive-odd))

(defn describe [value]
  (case (type value)
    java.lang.String :text
    clojure.lang.Keyword :keyword
    clojure.lang.PersistentVector :vector
    :other))

(defn countdown [start]
  (loop [remaining start
         events []]
    (if (zero? remaining)
      (conj events :launch)
      (recur (dec remaining) (conj events remaining)))))

(defn guarded-divide [numerator denominator]
  (try
    (/ numerator denominator)
    (catch ArithmeticException exception
      {:error :division-by-zero
       :message (.getMessage exception)})
    (finally
      (println "division attempted"))))

(defn validation-errors [{:keys [id city payload] :as launch}]
  (for [[field valid?] [[:id (integer? id)]
                       [:city (string? city)]
                       [:payload (some? payload)]]
        :when (not valid?)]
    {:field field :launch launch}))

(defn summarize
  ([values]
   (summarize values {}))
  ([values {:keys [separator] :or {separator ", "}}]
   (str/join separator (map render values))))

(defmulti destination :type)
(defmethod destination :orbital [{:keys [altitude]}]
  (str altitude " km"))
(defmethod destination :terrestrial [{:keys [city]}]
  city)
(defmethod destination :default [value]
  (pr-str value))

(defn destructuring-demo
  [{:keys [name roles] :or {roles []} :as person}
   [first-item & more :as items]]
  {:name name
   :admin? (contains? (set roles) :admin)
   :first first-item
   :rest more
   :counts [(count person) (count items)]})

(def sample-launch
  (->Launch 42 default-city {:crew ["Mae" "Sally"]}))

(comment
  ; Rich forms stay inert while remaining readable and balanced.
  (with-locale :ja
    (render sample-launch {:locale *locale*}))
  (increment-all [1 2 3])
  (pair->map :λ :🚀)
  (rest-counter 1 2 3 4)
  (re-find multiline-pattern launch-copy)
  (when-let+ (:city nested-data)
    (println "city found"))
  (throw (ex-info "closed fixture" {:kind ::complete})))
