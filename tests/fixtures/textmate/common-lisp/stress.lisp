;;;; Common Lisp TextMate stress fixture — café λ 東京 🚀 𝌆
;;;; Comments, reader syntax, definitions, macros, types, and format strings.

#| Outer block comment.
   #| Nested block comment with TODO and 🚀. |#
   The outer comment resumes here.
|#

(defpackage #:mark-stress
  (:use #:cl)
  (:nicknames #:mark.stress)
  (:export #:run-demo #:widget #:widget-name #:*default-widget*))
(in-package #:mark-stress)

;;; Definition-name and style-guide scopes.
(defconstant +limit+ 42)
(defparameter *default-widget* "café 🚀")
(defvar *cache* (make-hash-table :test #'equal))
(define-symbol-macro current-package *package*)

(deftype small-index () '(integer 0 255))
(defstruct (point (:constructor make-point (x y)))
  (x 0 :type integer)
  (y 0 :type integer))

(defclass widget (standard-object)
  ((name :initarg :name :accessor widget-name :type string)
   (state :initform :ready :accessor widget-state)
   (count :initform 0 :accessor widget-count)))

(define-condition widget-error (simple-error)
  ((widget :initarg :widget :reader error-widget))
  (:report (lambda (condition stream)
             (format stream "Bad widget: ~A" (error-widget condition)))))

(defgeneric render (object &key stream))
(defmethod render ((object widget) &key (stream *standard-output*))
  (format stream "~A [~A]~%" (widget-name object) (widget-state object)))

;;; Lambda lists and declarations.
(defun combine (required &optional (optional 10 supplied-p)
                &rest rest
                &key (scale 2) verbose
                &allow-other-keys
                &aux (total (+ required optional)))
  (declare (type integer required optional scale)
           (ignorable supplied-p verbose)
           (dynamic-extent rest)
           (optimize (speed 2) (safety 1) debug))
  (* scale (+ total (reduce #'+ rest :initial-value 0))))

(defmacro with-widget ((name &key (state :ready)) &body body)
  `(let ((,name (make-instance 'widget :name ,(string name))))
     (setf (widget-state ,name) ,state)
     ,@body))

(define-modify-macro multiplyf (factor) *)
(define-compiler-macro combine (&whole form &rest arguments)
  (declare (ignore arguments))
  form)

;;; Numeric constants: integers, ratios, floats, radices, signs, and exponents.
(defparameter *numbers*
  '(0 -1 +2 3/4 -5/7 1.25 .5 2. 6.02e23
    1.0s0 2.0f0 3.0d0 4.0l0
    #b101010 #o755 #xCAFE #16rBEEF #36rZ))

;;; Characters, strings, escapes, and FORMAT directives.
(defparameter *characters* '(#\A #\Space #\Newline #\λ #\🚀))
(defparameter *message*
  "Escaped quote: \"; slash: \\; café 東京 🚀")
(defparameter *formatted*
  "~A ~S ~D ~4,'0X ~:[no~;yes~] ~{~A~^, ~} ~/custom/")

(defun format-demo (items enabled)
  (format nil *formatted* "λ" items 42 42 enabled items))

;;; Quote, function quote, backquote, comma, splice, and package symbols.
(defparameter *quoted* '(alpha :keyword cl:car mark-stress::private))
(defparameter *template*
  `(root ,(+ 1 2) ,@(list 'café '東京) ,. (list 'tail)))
(defparameter *function* #'render)

;;; Sharpsign reader constructs covered by punctuation rules.
(defparameter *vector* #(1 2 3))
(defparameter *array* #2A((1 2) (3 4)))
(defparameter *bits* #*101001)
(defparameter *sized-bits* #8*101)
(defparameter *uninterned* '#:temporary)
(defparameter *pathname* #P"/tmp/café.txt")
(defparameter *complex* #C(3 4))
(defparameter *structure* #S(point :x 1 :y 2))
(defparameter *labelled* '#1=(a b . #1#))
#+sbcl (defparameter *implementation* :sbcl)
#-sbcl (defparameter *implementation* :portable)

;;; Special operators and binding forms.
(defun special-forms (value)
  (block done
    (let* ((first value)
           (second (if (plusp first) first (- first))))
      (labels ((walk (n)
                 (if (zerop n)
                     (return-from done second)
                     (walk (1- n)))))
        (flet ((local (x) (* x x)))
          (function local)
          (progn
            (setq second (local second))
            (the integer (walk 2))))))))

(defun control-transfer (tag value)
  (catch tag
    (unwind-protect
         (tagbody
          start
            (when value (go finish))
          finish
            (throw tag value))
      (values))))

;;; Macro families and looping constructs.
(defun iteration-demo (sequence)
  (let ((sum 0))
    (dolist (item sequence sum)
      (incf sum item))
    (dotimes (index 3)
      (decf sum index))
    (loop for item in sequence
          for index from 0
          when (evenp item) collect (cons index item)
          finally (return sum))))

(defun branching-demo (value)
  (cond
    ((null value) :empty)
    ((typep value 'string) :text)
    (t (case value
         ((0) :zero)
         ((1 2) :small)
         (otherwise :other)))))

(defun type-branching (value)
  (etypecase value
    (integer (1+ value))
    (string (length value))
    (sequence (count-if #'identity value))))

;;; Conditions, restarts, handlers, and side-effecting functions.
(defun guarded-render (object)
  (restart-case
      (handler-case
          (render object)
        (type-error (condition)
          (warn "Type error: ~A" condition)
          (error 'widget-error :widget object)))
    (use-default ()
      :report "Render the default widget"
      (render (make-instance 'widget :name *default-widget*)))))

(defun cache-value (key producer)
  (multiple-value-bind (value present-p) (gethash key *cache*)
    (if present-p
        value
        (setf (gethash key *cache*) (funcall producer)))))

;;; Accessors, pure functions, and side-effecting support function lists.
(defun collection-demo (items)
  (let* ((copy (copy-list items))
         (mapped (mapcar #'1+ copy))
         (filtered (remove-if-not #'evenp mapped))
         (sorted (stable-sort filtered #'<)))
    (pushnew 42 sorted)
    (nreverse sorted)))

(defun stream-demo (pathname)
  (with-open-file (stream pathname
                          :direction :output
                          :if-exists :supersede)
    (write-line "café 東京 🚀" stream)
    (finish-output stream)
    (file-position stream)))

;;; Built-in classes, type specifiers, and condition names.
(declaim (ftype (function (integer) integer) special-forms))
(check-type *cache* hash-table)
(assert (typep *default-widget* 'base-string))
(defparameter *type-names*
  '(array bit-vector sequence integer real number ratio
    standard-object standard-class structure-object
    unsigned-byte signed-byte fixnum bignum double-float
    simple-array simple-vector simple-string compiled-function))
(defparameter *condition-names*
  '(condition warning style-warning serious-condition error
    simple-error type-error arithmetic-error division-by-zero
    package-error file-error stream-error end-of-file))

;;; Earmuffs, plus constants, REPL history variables, and dotted lists.
(defparameter +unicode-label+ "λ 東京 𝌆")
(defparameter *special-state* :ready)
(defparameter *history-symbols* '(* ** *** + ++ +++ / // ///))
(defparameter *dotted-pair* '(left . right))

;;; Local function definition names.
(defun local-definitions (value)
  (flet ((square (x) (* x x)))
    (labels ((recur (x)
               (if (zerop x) 0 (+ (square x) (recur (1- x))))))
      (macrolet ((twice (form) `(+ ,form ,form)))
        (twice (recur value))))))

;;; Final integrated form.
(defun run-demo ()
  (with-widget (demo :state :ready)
    (render demo)
    (list (combine 1 2 3 4 :scale 2)
          (format-demo '("café" "東京" "🚀") t)
          (iteration-demo '(1 2 3 4))
          (branching-demo :ready)
          (collection-demo '(5 2 8 1))
          (local-definitions 4))))

(run-demo)
