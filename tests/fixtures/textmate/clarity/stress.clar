;; Stress Clarity contract
;; Unicode payload: café λ 東京 🚀 𝌆
;; Definitions exercise declarations, types, literals, and nested forms.

(define-constant owner tx-sender)
(define-constant zero u0)
(define-constant one u1)
(define-constant max-items u50)
(define-constant greeting u"café λ 東京 🚀 𝌆")
(define-constant ascii-label "clarity-fixture")
(define-constant empty-buffer 0x)
(define-constant marker 0xdeadbeef)

(define-data-var enabled bool true)
(define-data-var count uint u0)
(define-data-var signed-count int 0)
(define-data-var administrator principal tx-sender)
(define-data-var note (string-utf8 128) u"ready")
(define-data-var short-name (string-ascii 32) "mark")
(define-data-var payload (buff 64) 0xcafe)
(define-data-var maybe-owner (optional principal) none)
(define-data-var result (response uint uint) (ok u0))
(define-data-var recent (list 10 uint) (list u1 u2 u3))
(define-data-var profile {active: bool, score: uint}
  {active: true, score: u0})

(define-map balances principal uint)
(define-map allowances
  {owner: principal, spender: principal}
  {amount: uint, expires: uint})
(define-map messages
  uint
  (string-utf8 128))
(define-map records
  {id: uint, creator: principal}
  {memo: (string-ascii 32), digest: (buff 32)})

(define-fungible-token fixture-token u100000000)
(define-fungible-token uncapped-token)
(define-non-fungible-token collectible uint)
(define-non-fungible-token named-collectible (string-ascii 32))

(define-trait token-reader
  ((get-balance (principal) (response uint uint))
   (get-label (uint) (response (optional (string-utf8 128)) uint))))

(define-trait token-writer
  ((transfer (uint principal principal) (response bool uint))
   (set-label (uint (string-utf8 128)) (response bool uint))))

(use-trait reader .traits.token-reader)
(use-trait writer 'ST000000000000000000002AMW42H.external.writer)

(define-read-only (read-count)
  (ok (var-get count)))

(define-read-only (read-balance (account principal))
  (default-to
    u0
    (map-get? balances account)))

(define-read-only (read-allowance (from principal) (to principal))
  (match
    (map-get? allowances {owner: from, spender: to})
    entry (ok (get amount entry))
    (err u404)))

(define-read-only (unicode-label)
  (begin
    (print greeting)
    (ok u"café λ 東京 🚀 𝌆")))

(define-private (is-owner (candidate principal))
  (is-eq candidate owner))

(define-private (checked-add (left uint) (right uint))
  (let ((sum (+ left right)))
    (if (<= sum u100000000)
        (ok sum)
        (err u400))))

(define-private (remember (value uint))
  (let ((old (var-get recent))
        (next (unwrap! (as-max-len? (append old value) u10) (err u413))))
    (begin
      (var-set recent next)
      (ok next))))

(define-private (hash-note (text (string-utf8 128)))
  (sha256 text))

(define-public (set-enabled (value bool))
  (begin
    (asserts! (is-owner tx-sender) (err u401))
    (var-set enabled value)
    (ok value)))

(define-public (set-note (value (string-utf8 128)))
  (begin
    (asserts! (var-get enabled) (err u503))
    (var-set note value)
    (print value)
    (ok true)))

(define-public (deposit (amount uint))
  (let ((current (default-to u0 (map-get? balances tx-sender)))
        (updated (+ current amount)))
    (begin
      (asserts! (> amount u0) (err u400))
      (map-set balances tx-sender updated)
      (var-set count (+ (var-get count) u1))
      (try! (remember amount))
      (ok updated))))

(define-public (withdraw (amount uint))
  (let ((current (default-to u0 (map-get? balances tx-sender))))
    (begin
      (asserts! (>= current amount) (err u422))
      (map-set balances tx-sender (- current amount))
      (ok (- current amount)))))

(define-public (approve (spender principal) (amount uint) (expires uint))
  (begin
    (map-set allowances
      {owner: tx-sender, spender: spender}
      {amount: amount, expires: expires})
    (ok true)))

(define-public (revoke (spender principal))
  (begin
    (map-delete allowances {owner: tx-sender, spender: spender})
    (ok true)))

(define-public (mint-token (recipient principal) (amount uint))
  (begin
    (asserts! (is-owner tx-sender) (err u401))
    (ft-mint? fixture-token amount recipient)))

(define-public (move-token (amount uint) (sender principal) (recipient principal))
  (begin
    (asserts! (is-eq tx-sender sender) (err u403))
    (ft-transfer? fixture-token amount sender recipient)))

(define-public (mint-collectible (id uint) (recipient principal))
  (begin
    (asserts! (is-owner tx-sender) (err u401))
    (nft-mint? collectible id recipient)))

(define-public (move-collectible (id uint) (sender principal) (recipient principal))
  (nft-transfer? collectible id sender recipient))

(define-public (store-message (id uint) (message (string-utf8 128)))
  (begin
    (asserts! (< id max-items) (err u413))
    (map-set messages id message)
    (ok {id: id, saved: true})))

(define-read-only (list-tools (needle uint))
  (let ((items (list u1 u2 u3 u5 u8 u13)))
    (ok {length: (len items),
         index: (index-of? items needle),
         member: (is-some (index-of? items needle))})))

(define-read-only (buffer-tools)
  (ok {size: (len marker),
       digest: (hash160 marker),
       sliced: (slice? marker u0 u2)}))

(define-read-only (math-tools (value uint))
  (ok {square: (* value value),
       quotient: (/ value (+ value u1)),
       remainder: (mod value u7),
       root: (sqrti value),
       power: (pow value u2)}))

(define-read-only (logic-tools (a bool) (b bool))
  (ok {both: (and a b),
       either: (or a b),
       inverse: (not a),
       differs: (xor a b)}))

(define-read-only (principal-tools (who principal))
  (ok {standard: (is-standard who),
       contract: (contract-of reader),
       current: current-contract}))

(define-read-only (chain-context)
  (ok {height: block-height,
       burn: burn-block-height,
       sender: tx-sender,
       caller: contract-caller,
       chain: chain-id}))

(define-public (reset)
  (begin
    (asserts! (is-owner tx-sender) (err u401))
    (var-set count zero)
    (var-set signed-count 0)
    (var-set maybe-owner (some owner))
    (map-delete balances tx-sender)
    (ok one)))

;; Final nested, multiline form verifies state closure at EOF.
(define-read-only (summary (who principal))
  (let ((balance (default-to u0 (map-get? balances who)))
        (status (if (var-get enabled) u"enabled" u"disabled")))
    (ok {account: who,
         balance: balance,
         status: status,
         greeting: greeting})))
