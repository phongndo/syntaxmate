#!/usr/bin/env hy
; A small observatory queue service used to exercise the complete Hy grammar.
; Unicode data stays realistic: Երևան, naïve café, Διάστημα, 東京, 🔭, and 𐍈.
; The fixture favors readable application code over synthetic token repetition.
(import csv)
(import json)
(import math)
(import pathlib)
(import statistics)
(import datetime [datetime timezone])
(import collections [defaultdict])

(setv APP-NAME "Night Queue 🔭")
(setv VERSION "2.4.0")
(setv MISSING None)
(setv ENABLED True)
(setv DISABLED False)
(setv DEFAULT-LIMIT 12)
(setv MASK #b11110000)
(setv FILE-MODE #o640)
(setv COLOR #x2A7FFF)
(setv SIDEREAL-DAY 23.934)

(defclass QueueError [Exception]
  "Raised when an observing request cannot be normalized.")

(defclass Target []
  (defn __init__ [self name ra dec &optional [exposure 30.0] [repeats 1]]
    (setv self.name name)
    (setv self.ra ra)
    (setv self.dec dec)
    (setv self.exposure exposure)
    (setv self.repeats repeats))

  (defn duration [self]
    (* self.exposure self.repeats))

  (defn label [self]
    f"{self.name} ({self.ra:.3f}, {self.dec:.3f})"))

(defn clamp [value lower upper]
  (max lower (min upper value)))

(defn parse-number [text &optional default]
  (try
    (float (.strip text))
    (except [ValueError TypeError]
      default)))

(defn normalize-name [value]
  (setv clean (.strip (str (or value "unnamed"))))
  (if clean clean "unnamed"))

(defn normalize-row [row]
  (setv name (normalize-name (.get row "name")))
  (setv ra (parse-number (.get row "ra") 0.0))
  (setv dec (parse-number (.get row "dec") 0.0))
  (setv exposure (clamp (parse-number (.get row "exposure") 30) 1 900))
  (setv repeats (int (clamp (parse-number (.get row "repeats") 1) 1 20)))
  (Target name ra dec exposure repeats))

(defn load-targets [path]
  (with [handle (open path :encoding "utf-8")]
    (lfor row (csv.DictReader handle)
      (normalize-row row))))

(defn group-targets [targets]
  (setv groups (defaultdict list))
  (for [target targets]
    (.append (get groups (if (< target.dec 0) :south :north)) target))
  groups)

(defn total-duration [targets]
  (sum (gfor target targets (.duration target))))

(defn average-exposure [targets]
  (setv samples (lfor target targets target.exposure))
  (if samples (statistics.fmean samples) 0.0))

(defn visible? [target latitude]
  (and (>= target.dec (- latitude 90))
       (<= target.dec (+ latitude 90))
       (not (= target.name "maintenance"))))

(defn select-visible [targets latitude]
  (lfor target targets :if (visible? target latitude) target))

(defn priority-key [target]
  (, (- target.repeats) target.exposure (.casefold target.name)))

(defn schedule [targets latitude &optional [limit DEFAULT-LIMIT]]
  (cut (sorted (select-visible targets latitude) :key priority-key)
       0 limit))

(defn status-bits [ready tracking guiding]
  (setv bits 0)
  (when ready (setv bits (| bits #b0001)))
  (when tracking (setv bits (| bits #b0010)))
  (when guiding (setv bits (| bits #b0100)))
  (& bits MASK))

(defn quality [signal noise saturated]
  (cond
    saturated :saturated
    (<= noise 0) :unknown
    (> (/ signal noise) 5) :good
    True :poor))

(defn retry [operation &optional [attempts 3]]
  (setv last-error None)
  (for [attempt (range attempts)]
    (try
      (return (operation))
      (except [OSError :as error]
        (setv last-error error)
        (print f"attempt {(+ attempt 1)} failed: {error}"))))
  (raise (QueueError (str last-error))))

(defn encode-target [target]
  {"name" target.name
   "coordinates" [target.ra target.dec]
   "exposure" target.exposure
   "repeats" target.repeats
   "duration" (.duration target)})

(defn render-report [targets]
  (setv generated (.isoformat (datetime.now timezone.utc)))
  (setv rows (lfor target targets (encode-target target)))
  (json.dumps
    {"application" APP-NAME
     "version" VERSION
     "generated" generated
     "count" (len rows)
     "targets" rows}
    :ensure-ascii False
    :indent 2))

(defn write-report [path targets]
  (setv destination (pathlib.Path path))
  (.write-text destination (render-report targets) :encoding "utf-8")
  destination)

(defn describe [target]
  (match target.name
    "maintenance" "reserved engineering window"
    "M42" "Orion nebula"
    "東京" "Tokyo calibration field"
    _ (.label target)))

(defn coordinate-pairs [targets]
  (gfor target targets (, target.ra target.dec)))

(defn unpack-demo [payload]
  (setv #** metadata payload)
  (setv #* values (.get metadata "values" []))
  (, metadata values))

(defmacro with-banner [#* body]
  `(do
     (print ~APP-NAME)
     ~@body))

(defreader q [form]
  `(quote ~form))

(setv SAMPLE
  [{"name" "M42" "ra" "83.822" "dec" "-5.391" "exposure" "45" "repeats" "3"}
   {"name" "Արագած" "ra" "12.5" "dec" "40.5" "exposure" "60" "repeats" "2"}
   {"name" "東京 🚀" "ra" "120" "dec" "35" "exposure" "10" "repeats" "1"}
   {"name" "𐍈 field" "ra" "240" "dec" "-20" "exposure" "90" "repeats" "4"}
   {"name" "maintenance" "ra" "0" "dec" "0" "exposure" "1" "repeats" "1"}])

(setv TARGETS (lfor row SAMPLE (normalize-row row)))
(setv PLAN (schedule TARGETS 42.5 :limit 4))
(setv NORTH (get (group-targets TARGETS) :north))
(setv SOUTH (get (group-targets TARGETS) :south))

(with-banner
  (print "plan:" (lfor target PLAN (describe target)))
  (print "duration:" (round (total-duration PLAN) 2))
  (print "average:" (round (average-exposure PLAN) 2))
  (print "hemispheres:" (len NORTH) (len SOUTH))
  (print "status:" (status-bits True True False)))

(assert (all (map visible? PLAN (repeat 42.5))))
(assert (any (gfor target TARGETS (= target.name "M42"))))
(assert (is-not MISSING False))
(assert (in :north (group-targets TARGETS)))

; Isolated forms cover operator and control spellings used by macro-heavy code.
(setv arithmetic (+ (- (* 7 6) (/ 8 2)) (// 9 2) (% 9 2) (** 2 3)))
(setv shifted (| (<< 1 4) (>> 64 2)))
(setv compared (and (!= arithmetic 0) (is-not shifted None)))
(setv quoted '(alpha :beta 42))
(setv quasiquoted `(alpha ~arithmetic ~@PLAN))
(print arithmetic shifted compared quoted quasiquoted)
