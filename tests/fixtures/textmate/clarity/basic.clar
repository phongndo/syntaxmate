;; Basic Clarity fixture: café λ 東京 🚀 𝌆
(define-constant contract-owner tx-sender)
(define-data-var counter uint u0)
(define-map balances principal uint)
(define-fungible-token credits u1000000)
(define-non-fungible-token badge uint)
(define-trait greeter
  ((greet (principal (string-utf8 64)) (response bool uint))))
(use-trait remote-greeter .traits.greeter)
(define-read-only (label (who principal))
  (ok u"café λ 東京 🚀 𝌆"))
(define-public (increment (amount uint))
  (let ((before (var-get counter))
        (after (+ before amount)))
    (begin
      (var-set counter after)
      (print {who: tx-sender, value: after})
      (ok after))))
(define-private (positive? (value int))
  (> value 0))

