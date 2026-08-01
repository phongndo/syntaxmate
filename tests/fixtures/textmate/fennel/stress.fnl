; Observatory queue processor used to exercise the Fennel grammar.
; Unicode labels: naïve café, Διάστημα, 世界; astral symbols: 🔭 🛰️ 🌌.
; Operators submit comma-separated right ascension, declination, exposure, repeat, and filter fields through load-targets.
; normalize-target applies conservative defaults so partially entered queue records remain safe to inspect before scheduling.
; Coordinates are represented in decimal degrees; callers perform hour-angle conversion before constructing target tables.
; The in-memory logger deliberately implements the same write method as an ordinary Lua file handle for dependency injection.
; Scheduling ignores maintenance records while retaining their position, which keeps imported observing plans reproducible.
; Exposure totals include repeats but omit readout overhead; the production service adds camera-specific timing afterward.
; Signal quality is acceptable only when a sample is neither saturated nor faint and exceeds five times the measured noise.
; Bit masks reserve the upper byte for a detector channel and the lower byte for calibration and acquisition status flags.
; Pagination uses floor division and remainder so command-line summaries can show complete pages and a partial final page.
; Threading macros keep the report pipeline readable while optional threading safely traverses absent observer addresses.
; choose-action demonstrates structural matching over weather and dome state without coupling the scheduler to UI strings.
; wait-until cooperatively yields instead of sleeping, allowing telescope, enclosure, and weather coroutines to share a loop.
; guarded-call normalizes protected-call results into a value/error pair suitable for logs and unattended retry decisions.
; Status updates mutate one shared table intentionally because monitoring clients retain references to the active run state.
; sorted-copy preserves incoming target order while producing deterministic reports for operators and golden-file reviews.
; Demo records include emission-nebula, cluster, and maintenance entries with realistic coordinates and exposure durations.
; The report string spans physical lines to exercise multiline string state, escaped quotes, tabs, and a terminal backslash.
; Assertions document fixture invariants and also cover equality, comparison, member access, function calls, and literals.
; This corpus favors representative observatory code over generated repetition while keeping process-cold measurements stable.
; End-to-end examples preserve accented, CJK, Greek, and astral text so every tokenizer path must honor UTF-8 boundaries.

(local math math)
(local string string)
(local table table)
(local os os)
(local io io)

(global observatory
  {:name "North Ridge 🔭"
   :active true
   :retry-limit 4
   :exposure-scale 1.25E+2
   :temperature -12.5
   :missing nil})

(local default-filters
  [{:name "luminance" :wavelength 540}
   {:name "hydrogen-α" :wavelength 656.28}
   {:name "oxygen-III" :wavelength 500.7}])

(fn clamp [value low high]
  (math.max low (math.min high value)))

(fn round [value]
  (math.floor (+ value 0.5)))

(λ square [x]
  (* x x))

(fn angular-distance [a b]
  (let [delta (- a b)]
    (math.sqrt (square delta))))

(fn normalize-target [target]
  (let [name (or (. target :name) "unnamed")
        exposure (clamp (or (. target :exposure) 30) 1 900)
        repeats (math.max 1 (or (. target :repeats) 1))]
    {:name name
     :coordinates [(. target :ra) (. target :dec)]
     :exposure exposure
     :repeats repeats
     :filter (or (. target :filter) :luminance)}))

(fn parse-coordinate [text]
  (let [trimmed (string.gsub text "^%s*(.-)%s*$" "%1")
        number (tonumber trimmed)]
    (assert number (string.format "bad coordinate: %q" text))
    number))

(fn load-targets [path]
  (with-open [handle (assert (io.open path "r"))]
    (icollect [line (handle:lines)]
      (let [(name ra dec exposure)
            (string.match line "([^,]+),([^,]+),([^,]+),?(.*)")]
        (normalize-target
          {:name name
           :ra (parse-coordinate ra)
           :dec (parse-coordinate dec)
           :exposure (or (tonumber exposure) 30)})))))

(fn log-event [logger level message fields]
  (let [timestamp (os.date "!%Y-%m-%dT%H:%M:%SZ")
        suffix (if fields
                   (string.format " %s" fields)
                   "")]
    (logger:write
      (string.format "%s [%s] %s%s\n"
                     timestamp
                     (string.upper level)
                     message
                     suffix))))

(fn make-memory-logger []
  (let [lines []
        logger {}]
    (tset logger :write
      (fn [self line]
        (table.insert lines line)
        self))
    (tset logger :lines lines)
    logger))

(fn group-by-filter [targets]
  (collect [_ target (ipairs targets)]
    (values (. target :filter) target)))

(fn indexed-names [targets]
  (icollect [index target (ipairs targets)]
    (string.format "%02d:%s" index (. target :name))))

(fn total-exposure [targets]
  (var seconds 0)
  (each [_ target (ipairs targets)]
    (set seconds
         (+ seconds
            (* (. target :exposure)
               (. target :repeats)))))
  seconds)

(fn quality-flags [sample]
  (let [signal (. sample :signal)
        noise (. sample :noise)
        saturated? (>= signal 65535)
        faint? (< signal 120)
        usable? (and (not saturated?)
                     (not faint?)
                     (> signal (* noise 5)))]
    {:saturated saturated?
     :faint faint?
     :usable usable?
     :ratio (if (= noise 0) math.huge (/ signal noise))}))

(fn bit-mask [channel flags]
  (let [base (lshift channel 8)
        combined (bor base flags)
        toggled (bxor combined 3)]
    (band (bnot toggled) 65535)))

(fn pagination [count page-size]
  {:pages (// (+ count (- page-size 1)) page-size)
   :remainder (% count page-size)})

(fn describe [target]
  (-> target
      (normalize-target)
      (. :name)
      (string.upper)))

(fn describe-all [targets]
  (->> targets
       (icollect [_ target (ipairs targets)] (describe target))
       (table.concat ", ")))

(fn optional-city [payload]
  (-?> payload
       (. :observer)
       (. :address)
       (. :city)))

(fn optional-initial [payload]
  (-?>> payload
        (. :observer)
        (. :name)
        (string.sub 1 1)))

(fn calibration-name [frame]
  (or (?. frame :metadata :calibration :name)
      "unknown"))

(fn choose-action [state]
  (match state
    {:weather :clear :dome :open} :observe
    {:weather :clear} :open-dome
    {:weather weather} (.. "wait-for-" weather)
    _ :sleep))

(fn schedule [targets logger]
  (var completed 0)
  (for [index 1 (# targets)]
    (let [target (. targets index)]
      (if (= (. target :name) "maintenance")
          (log-event logger "info" "skipping maintenance" nil)
          (do
            (log-event logger "info" "capturing" (. target :name))
            (set completed (+ completed 1))))))
  completed)

(fn wait-until [predicate timeout]
  (let [started (os.clock)]
    (while (and (not (predicate))
                (< (- (os.clock) started) timeout))
      (coroutine.yield :waiting))
    (predicate)))

(fn guarded-call [f ...]
  (let [(ok value) (pcall f ...)]
    (if ok
        (values value nil)
        (values nil (tostring value)))))

(fn update-status! [state key value]
  (tset state key value)
  (set (. state :updated-at) (os.time))
  state)

(fn sorted-copy [items]
  (doto (table.move items 1 (# items) 1 {})
    (table.sort)))

(fn build-run [targets]
  (let [logger (make-memory-logger)
        state {:phase :starting :attempt 0}]
    (update-status! state :phase :running)
    {:logger logger
     :state state
     :count (schedule targets logger)
     :seconds (total-exposure targets)}))

(local demo-targets
  [{:name "M42 — Orion 🌌"
    :ra 83.822
    :dec -5.391
    :exposure 45
    :repeats 3
    :filter :hydrogen-alpha}
   {:name "NGC 869"
    :ra 34.75
    :dec 57.15
    :exposure 90
    :repeats 2
    :filter :oxygen-III}
   {:name "maintenance"
    :ra 0
    :dec 0
    :exposure 1
    :repeats 1}])

(local escaped
  "first report line
second line with \"quoted text\"
third line with a tab:\tand a backslash: \\")

(local report
  (build-run demo-targets))

(assert (= (. report :count) 2))
(assert (= (type (. report :logger)) "table"))
(assert (> (. report :seconds) 0))
(print (describe-all demo-targets))
(print escaped)
