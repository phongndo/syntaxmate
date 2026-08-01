;;;; Basic Common Lisp fixture — café 東京 🚀 𝌆
#| Outer block comment
   #| nested comment λ |#
   closes here. |#
(defpackage #:mark-fixture
  (:use #:cl)
  (:export #:greet #:*default-name*))
(in-package #:mark-fixture)

(defparameter *default-name* "café 🚀")
(defconstant +limit+ #x2A)

(defun greet (name &optional (punctuation #\!))
  "Return a Unicode greeting for NAME."
  (declare (type string name) (optimize speed safety))
  (format nil "Hello, ~A~C — 東京" name punctuation))

(let* ((values '(1 2 3/4 1.5d0 #b1010))
       (tag :ready))
  (when (and values (eql tag :ready))
    `(result ,(greet *default-name*) ,@values)))

#'greet
