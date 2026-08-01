#! /usr/bin/env racket
#lang racket

(require racket/list
         racket/match
         racket/string
         racket/format)
(provide summarize-readings
         render-report
         (struct-out reading)
         (struct-out station))

; Observatory report processor: naïve façade, 東京, and 🛰.
#| The fixture keeps every reader state closed.
   #| Nested block comments exercise recursive delimiters: λ and 🧪. |#
   Nothing in this paragraph is evaluated. |#

(struct reading (sensor value unit [quality #:mutable]) #:transparent)
(struct station [name location readings] #:transparent)
(define empty-reading #s(reading unknown 0 unknown-unit missing))

(define default-title 'nightly-sky-report)
(define escaped-text
  (format "tabs:\t newline:\n quote:\" hex:\x41; unicode:\u03bb ~a" 'done))
(define byte-prefix (bytes 79 66 83 10))
(define sensor-pattern #<<RX
^([[:alpha:]-]+):\\s*(-?[0-9.]+)$
RX
)
(define sensor-rx (pregexp sensor-pattern))
(define byte-rx (byte-pregexp byte-prefix))
(define command-rx byte-rx)

(define introduction #<<REPORT
This multiline report belongs to café Δ.
It tracks weather, seeing, and the telescope emoji 🔭.
REPORT
)

(define exact-count 42)
(define decimal-value -12.75e2)
(define rational-value 355/113)
(define complex-value 3-4i)
(define polar-value 2@1.5708)
(define binary-mask #b101101)
(define octal-mode #o755)
(define hexadecimal-color #x7fa0cc)
(define forced-exact #e1.25)
(define forced-inexact #i3/7)
(define positive-infinity +inf.0)
(define missing-number +nan.0)

(define whitespace #\space)
(define newline #\newline)
(define lambda-char #\u03bb)
(define satellite-char #\U1F6F0)
(define line-separator (string newline))
(define truth-table (vector #t #true #f #false))
(define fixed-values #4(10 20 30 40))
(define flonums (vector 1.0 2.5 -3.25))
(define fixnums (vector 1 2 3 5 8))

(define unit-table
  #hash((temperature . celsius)
        (humidity . percent)
        (wind . meters-per-second)))
(define identity-table
  #hasheq((primary . WX-01)
          (secondary . SKY-02)))
(define numeric-table
  #hasheqv((1 . one)
           (2 . two)))
(define literal-box #&immutable-sample)
(define latest-box (box 'not-sampled))

(define-values (minimum-quality maximum-quality)
  (values 0 100))

(define (clamp value [low minimum-quality] [high maximum-quality])
  (min high (max low value)))

(define (quality-label score #:verbose? [verbose? #f])
  (define label
    (cond
      [(>= score 90) 'excellent]
      [(>= score 70) 'good]
      [(>= score 40) 'fair]
      [else 'poor]))
  (if verbose?
      (format "~a (~a%)" label score)
      label))

(define (normalize-sensor-value value offset scale)
  (/ (- value offset) scale))

(define add-offset
  (lambda (amount)
    (lambda (value) (+ value amount))))

(define (parse-reading line)
  (match (regexp-match sensor-rx line)
    [(list _ sensor raw)
     (define parsed (string->number raw))
     (define key (string->symbol sensor))
     (define unit
       (hash-ref unit-table key 'raw))
     (and parsed
          (reading key parsed unit 'unchecked))]
    [_ #f]))

(define sample-text #<<SAMPLES
temperature: 18.75
humidity: 63
wind: 4.5
seeing: 1.2
invalid input
SAMPLES
)
(define sample-lines
  (string-split sample-text (string #\newline)))

(define samples
  (filter-map parse-reading sample-lines))

(define home-station
  (station 'North-Ridge
           '(48.8566 . 2.3522)
           samples))

(define (grade! item)
  (match-define (reading sensor value unit _) item)
  (define score
    (case sensor
      [(temperature) (clamp (- 100 (abs (- value 15))))]
      [(humidity) (clamp (- 100 (abs (- value 50))))]
      [(wind) (clamp (- 100 (* value 4)))]
      [else 75]))
  (set-reading-quality! item (quality-label score))
  item)

(define (usable? item)
  (real? (reading-value item)))

(define (summarize-readings items)
  (define accepted
    (filter usable? items))
  (define total
    (for/sum ([item (in-list accepted)])
      (reading-value item)))
  (values total accepted))

(define (render-reading item #:index [index 0])
  (define name (reading-sensor item))
  (define value (reading-value item))
  (format "~a. ~a = ~a" index name value))

(define (render-report site)
  (define-values (totals accepted)
    (summarize-readings (station-readings site)))
  (define rows
    (map render-reading accepted))
  (define summary
    (format "Total: ~a" totals))
  (define all-rows (cons summary rows))
  (string-join all-rows line-separator))

(define generated `(station title))
(define quoted-forms
  (list 'alpha
        '|symbol with spaces|
        generated
        #'syntax-object))

(define mixed-delimiters
  '{[alpha 1]
   [beta {2 3}]
   [gamma (list #:enabled #t #:mode 'night)]})

#;'(discarded datum #t)

(define dotted-pair '(left . right))
(define prefab-position #s(position 12 34))
(define case-note '#ci MixedCaseSymbol)

(when (pair? samples)
  (set-box! latest-box (car samples)))

(displayln introduction)
(displayln (render-report home-station))
(printf "Latest: ~s~n" (unbox latest-box))
(write lambda-char)
(write satellite-char)
(newline)
