; Observatory catalog example for Scheme tokenization.
; BMP text includes naïve, λ, Ж, and 東京; astral text includes 🔭 and 🛰️.

#| The catalog combines immutable observations with a small report pipeline.
   Nested comments are legal in this reader:
   #| calibration note: Ω remains a BMP symbol, while 🌌 is astral |#
   The outer comment closes on the following marker.
|#

(define catalog-name "Café λ Observatory 🔭")
(define release "2026.07")
(define tracing? #t)
(define archived? #f)
(define empty-flag #f)

; Characters exercise named, hexadecimal, ASCII, BMP, and astral spellings.
(define separator #\space)
(define line-end #\newline)
(define tabulator #\tab)
(define ascii-star #\*)
(define greek-lambda #\x03BB)
(define direct-letter #\é)
(define astral-rocket #\🚀)

; Exactness, radix prefixes, decimals, rationals, complexes, and exponents.
(define answer 42)
(define exact-answer #e42)
(define inexact-half #i0.5)
(define hexadecimal #x2A)
(define octal #o52)
(define binary #b101010)
(define ratio 355/113)
(define offset -17)
(define tiny 6.022e-23)
(define complex-offset 3+4i)

(define channels #(visible infrared ultraviolet "ραδιο" 🔭))
(define calibration #(#(0.0 1.0) #(0.1 0.9) #(0.2 0.8)))
(define packet #u8(0 1 2 127 128 254 255))

; A datum comment suppresses one complete, balanced definition.
#;(define obsolete-channel
    #(x-ray "retired" #u8(9 9 9)))

; A second datum comment discards a quoted migration plan.
#;'(migrate (old-port 4100) (new-port 4200))

(define empty-quote '())
(define compass-symbol 'north-east)
(define quoted-layout
  '(catalog
     (columns name right-ascension declination)
     (units text hours degrees)))

(define default-options
  `((title . ,catalog-name)
    (channels . ,(vector->list channels))
    (labels . ,@'(name magnitude epoch))
    (theme . "night 🌃")))

(define multiline-banner
  "First line: Observatório
Second line: 東京 station 🛰️
Third line closes this string.")

(define escaped-text
  "quote: \"catalog\", slash: \\, tab:\t, newline:\n")

(define-syntax when
  (syntax-rules ()
    ((when predicate expression ...)
     (if predicate
         (begin expression ...)))))

(define-syntax unless
  (syntax-rules ()
    ((unless predicate expression ...)
     (if (not predicate)
         (begin expression ...)))))

(define-syntax define-observation
  (syntax-rules ()
    ((define-observation identifier label magnitude)
     (define identifier
       (list (cons 'label label)
             (cons 'magnitude magnitude))))))

(define-observation vega "Vega α Lyrae" 0.03)
(define-observation rigel "Rigel β Orionis" 0.13)

(define (square value)
  (* value value))

(define (clamp value low high)
  (cond
    ((< value low) low)
    ((> value high) high)
    (else value)))

(define (classify-magnitude magnitude)
  (case (inexact->exact (floor magnitude))
    ((-2 -1) 'brilliant)
    ((0 1) 'bright)
    ((2 3 4) 'visible)
    (else 'faint)))

(define (make-counter initial)
  (let ((count initial))
    (lambda ()
      (set! count (+ count 1))
      count)))

(define next-serial (make-counter 1000))

(define (make-observation name magnitude . tags)
  (let* ((bounded (clamp magnitude -2.0 30.0))
         (kind (classify-magnitude bounded))
         (serial (next-serial)))
    `((serial . ,serial)
      (name . ,name)
      (magnitude . ,bounded)
      (class . ,kind)
      (tags . ,tags))))

(define observations
  (list
    (make-observation "Sirius 🐕" -1.46 'star 'southern)
    (make-observation "Vega λ" 0.03 'star 'northern)
    (make-observation "Andromeda 🌌" 3.44 'galaxy 'northern)
    (make-observation "東京 transient" 5.2 'candidate 'review)))

(define (observation-ref observation key)
  (let ((entry (assq key observation)))
    (if entry
        (cdr entry)
        #f)))

(define (bright? observation)
  (let ((magnitude (observation-ref observation 'magnitude)))
    (and (number? magnitude)
         (< magnitude 2.0))))

(define (interesting? observation)
  (or (bright? observation)
      (memq 'review (observation-ref observation 'tags))))

(define (filter predicate items)
  (cond
    ((null? items) '())
    ((predicate (car items))
     (cons (car items) (filter predicate (cdr items))))
    (else
     (filter predicate (cdr items)))))

(define (fold-left procedure seed items)
  (let loop ((result seed) (rest items))
    (if (null? rest)
        result
        (loop (procedure result (car rest))
              (cdr rest)))))

(define (total-brightness items)
  (fold-left
    (lambda (sum observation)
      (+ sum
         (square (- 10.0
                    (observation-ref observation 'magnitude)))))
    0.0
    items))

(define (print-observation observation)
  (display (observation-ref observation 'name))
  (display separator)
  (display (observation-ref observation 'class))
  (display separator)
  (write (observation-ref observation 'magnitude))
  (newline))

(define (print-report items)
  (begin
    (display multiline-banner)
    (newline)
    (for-each print-observation items)
    (display "score=")
    (write (total-brightness items))
    (newline)))

(define (lookup serial items)
  (letrec ((search
             (lambda (remaining)
               (cond
                 ((null? remaining) #f)
                 ((= serial
                     (observation-ref (car remaining) 'serial))
                  (car remaining))
                 (else (search (cdr remaining)))))))
    (search items)))

(define delayed-report
  (delay (filter interesting? observations)))

(define (countdown start)
  (do ((remaining start (- remaining 1))
       (accumulator '() (cons remaining accumulator)))
      ((zero? remaining) accumulator)
    (when tracing?
      (display remaining)
      (display #\space))))

(define (safe-divide numerator denominator)
  (if (zero? denominator)
      (values #f 'division-by-zero)
      (values (/ numerator denominator) #f)))

(define (dispatch command)
  (case command
    ((report) (print-report (force delayed-report)))
    ((countdown) (write (countdown 3)))
    ((config) (write default-options))
    (else (display "unknown command"))))

(unless archived?
  (let ((selected (force delayed-report)))
    (if (pair? selected)
        (print-report selected)
        (display "no observations"))))

(when (and tracing? (not empty-flag))
  (display escaped-text)
  (newline)
  (write (lookup 1001 observations))
  (newline))

; Every string, comment, quotation, datum, and list is closed before EOF.
(dispatch 'config)
