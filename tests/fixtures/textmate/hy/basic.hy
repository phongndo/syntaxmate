#!/usr/bin/env hy
; Compact Hy grammar coverage: Երևան, λ, 東京, and 🚀.
(import pathlib)
(setv greeting "Բարև\nաշխարհ" city "東京" rocket "🚀")
(setv counts [0 17 3.5 #x2A #o52 #b101010])
(setv options {:quiet False :limit 3 :missing None})

(defn welcome [name]
  (print f"{greeting}, {name}! {rocket}")
  (if (and name (not-in name ["n/a" ""] ))
    (.upper name)
    "anonymous"))

(defclass Greeter []
  (defn __init__ [self prefix]
    (setv self.prefix prefix))
  (defn render [self value]
    (format "{}: {}" self.prefix value)))

(setv doubled (lfor n (range 6) :if (>= n 2) (* n 2)))
(setv flags (| #b0011 #b0100))
(print (welcome "Ada") doubled flags options)
