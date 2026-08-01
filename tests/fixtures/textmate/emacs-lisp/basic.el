;;; basic.el --- Small multilingual greeting fixture -*- lexical-binding: t; -*-

(require 'cl-lib)

(defgroup mark-fixture nil "Settings for λ and 🚀 greetings." :group 'applications)
(defcustom mark-fixture-prefix "Hello" "Greeting prefix." :type 'string)
(defface mark-fixture-face '((t :weight bold :foreground "DeepSkyBlue"))
  "Face used for fixture messages." :group 'mark-fixture)
(defconst mark-fixture-codes [#b1010 #x2a 3.5 ?λ ?🚀])

;;;###autoload
(defun mark-fixture-greet (name &optional loud)
  "Return a greeting for NAME.
With LOUD non-nil, uppercase it; see `upcase' and \\[mark-fixture-greet]."
  (interactive "sName: ")
  (let* ((message (format "%s, %s — λ 🚀" mark-fixture-prefix name))
         (result (if loud (upcase message) message)))
    (when (called-interactively-p 'interactive)
      (message "%s" result))
    result))

(cl-loop for item across mark-fixture-codes collect item)
`(:greeting ,(mark-fixture-greet "世界") :keys [C-c M-g] ,@'(t nil))

(provide 'basic)
;;; basic.el ends here
