; Compact Scheme sampler: café λ 😀
#| A closed block comment with Ω and 🚀. |#
(define greeting "hello, 世界 🌍")
(define enabled? #t)
(define samples '(#f #\space #\λ 42 3.5 #x2a #o52 #b101010))
(define vector-data #(alpha 7 "β"))
(define bytes #u8(0 127 255))

(define-syntax when
  (syntax-rules ()
    ((when test body ...)
     (if test (begin body ...)))))

(define (describe name . tags)
  (let ((message `(record ,name ,@tags)))
    (cond
      ((null? tags) 'empty)
      (else message))))

#;(define discarded "datum comment")
(when (and enabled? (number? 42))
  (display (describe greeting 'unicode 'ready))
  (newline))

(case (vector-ref vector-data 1)
  ((7) "seven")
  (else "other"))
