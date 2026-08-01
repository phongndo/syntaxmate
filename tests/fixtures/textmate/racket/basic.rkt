#lang racket

; A tiny Unicode greeter: café λ 🚀
(struct guest (name [visits #:mutable]) #:transparent)
(define default-name 'world)
(define-values (first-count bonus) (values #x2a 3/2))

(define (greet who [punctuation 'bang] #:loud? [loud? #f])
  (define message (format "Hello, ~a~a" who punctuation))
  (if loud? (string-upcase message) message))

(define sample
  (hash 'name default-name
        'numbers (vector #b1010 #o17 first-count 2.5+3i)))

#| A closed block comment
   with BMP Ω and astral 🧭. |#
(for ([name (in-list (list 'Ada default-name))])
  (displayln (greet name 'bang #:loud? #t)))
#;(displayln 'discarded-datum)
(displayln (hash-ref sample 'numbers))
