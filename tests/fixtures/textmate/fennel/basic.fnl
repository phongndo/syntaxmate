; A compact Fennel service with Unicode: café and an astral moon 🌙.
(local config
  {:host "localhost"
   :port 8080
   :ratio 6.25E-2
   :enabled true
   :fallback nil})

(fn greet [name]
  (let [message (.. "héllo, " name "!\n")]
    (print message)
    message))

(fn summarize [values]
  (var total 0)
  (each [index value (ipairs values)]
    (when (and (> value 0) (not= value 13))
      (set total (+ total value))))
  {:count (# values) :total total})

(local result (summarize [1 2 -3 4]))
(assert (= (. result :total) 7))
(greet "世界 🚀")
