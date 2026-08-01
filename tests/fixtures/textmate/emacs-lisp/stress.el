#!/usr/bin/env -S emacs --script
;;; stress.el --- Stateful workspace dashboard fixture -*- lexical-binding: t; coding: utf-8 -*-

;; Copyright (C) 2026 Fixture Authors
;; Author: Syntax Fixture <fixture@example.invalid>
;; Keywords: tools, convenience, unicode
;; Package-Requires: ((emacs "29.1") (cl-lib "0.6"))
;;; Commentary:
;; This library exercises dashboard state, doc links such as
;; `stress-dashboard-open', \[stress-dashboard-refresh], and
;; \{stress-dashboard-mode-map}, plus café, 東京, λ, and astral 🚀 text.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'subr-x)
(require 'tabulated-list)

(defgroup stress-dashboard nil
  "A small dashboard used by syntax fixtures."
  :group 'tools
  :prefix "stress-dashboard-")
(defface stress-dashboard-title
  '((t :inherit variable-pitch :weight bold :height 1.2))
  "Face for the dashboard title."
  :group 'stress-dashboard)
(defface stress-dashboard-warning
  '((((class color) (min-colors 88)) :foreground "DarkOrange" :weight bold)
    (t :inherit warning))
  "Face for stale or failed jobs."
  :group 'stress-dashboard)
(defcustom stress-dashboard-root user-emacs-directory
  "Directory scanned by the dashboard."
  :type 'directory
  :group 'stress-dashboard)
(defcustom stress-dashboard-archives '(gnu melpa-stable melpa)
  "Package archives displayed in summaries."
  :type '(repeat symbol)
  :group 'stress-dashboard)
(defcustom stress-dashboard-refresh-seconds 2.5
  "Seconds between automatic refreshes."
  :type 'number
  :group 'stress-dashboard)
(defcustom stress-dashboard-visible-states '(ready plain failed stale)
  "States retained when a dashboard filter is active."
  :type '(set (const ready) (const plain) (const failed) (const stale))
  :group 'stress-dashboard)
(defconst stress-dashboard--numeric-samples
  [#b101101 #B0011 #x2A #Xff -17 +23 0.125 6.02e23 -2E-3]
  "Reader forms retained to exercise numeric highlighting.")
(defconst stress-dashboard--characters
  [?a ?\n ?\x3bb ?\u6771 ?\U0001F680 ?λ ?東 ?🚀]
  "Character literals, including Unicode and astral code points.")
(defconst stress-dashboard--state-labels
  '((ready . "Ready ✓") (plain . "Plain")
    (failed . "Failed ✗") (stale . "Stale…"))
  "Readable labels used in table rows and completion annotations.")

(defvar stress-dashboard--timer nil)
(defvar stress-dashboard--generation 0)
(defvar-local stress-dashboard--rows nil)
(defvar-local stress-dashboard--last-error nil)

(cl-defstruct (stress-dashboard-job
               (:constructor stress-dashboard-job-create))
  name path state duration tags metadata)
(defvar stress-dashboard-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "g") #'stress-dashboard-refresh)
    (define-key map (kbd "RET") #'stress-dashboard-visit)
    (define-key map (kbd "C-c C-r") #'stress-dashboard-restart)
    map)
  "Keymap for `stress-dashboard-mode'.")

(define-derived-mode stress-dashboard-mode tabulated-list-mode "Stress-Dashboard"
  "Major mode for a compact project dashboard.

Use \\[stress-dashboard-refresh] to refresh and
\\[stress-dashboard-visit] to visit the row at point.
The complete keymap is shown by \\{stress-dashboard-mode-map}."
  (setq-local tabulated-list-format
              [("Project" 24 t)
               ("State" 10 string-lessp)
               ("Seconds" 9 nil :right-align t)
               ("Tags" 24 nil)])
  (setq-local tabulated-list-padding 2)
  (setq-local revert-buffer-function #'stress-dashboard-refresh)
  (tabulated-list-init-header))
(defsubst stress-dashboard--safe-name (value)
  "Return VALUE as a trimmed display string."
  (string-trim (format "%s" (or value "unnamed"))))
(defmacro stress-dashboard--with-root (&rest body)
  "Evaluate BODY with `default-directory' set to the dashboard root."
  (declare (indent 0) (debug t))
  `(let ((default-directory (file-name-as-directory
                             (expand-file-name stress-dashboard-root))))
     ,@body))
(defun stress-dashboard--discover ()
  "Return jobs discovered below `stress-dashboard-root'."
  (stress-dashboard--with-root
    (cl-loop for path in (directory-files default-directory t "^[^.]")
             when (file-directory-p path)
             for name = (file-name-nondirectory path)
             for state = (if (file-exists-p (expand-file-name ".git" path))
                             'ready
                           'plain)
             collect (stress-dashboard-job-create
                      :name name
                      :path path
                      :state state
                      :duration (/ (float (length name)) 10)
                      :tags (if (string-match-p "test\\|spec" name)
                                '(test local)
                              '(local))
                      :metadata `((generation . ,stress-dashboard--generation)
                                  (unicode . "東京 λ 🚀"))))))
(defun stress-dashboard--normalize (job)
  "Normalize JOB and return a tabulated-list row."
  (pcase-let* (((cl-struct stress-dashboard-job
                           name path state duration tags metadata) job)
               (print-name (stress-dashboard--safe-name name))
               (state-name (or (alist-get state stress-dashboard--state-labels)
                               (symbol-name (or state 'unknown))))
               (tag-text (mapconcat #'symbol-name tags ", "))
               (`(,generation . ,_) (assq 'generation metadata)))
    (list path
          (vector
           (propertize print-name 'face 'stress-dashboard-title)
           (if (memq state '(failed stale))
               (propertize state-name 'face 'stress-dashboard-warning)
             state-name)
           (format "%.2f" (or duration 0.0))
           (format "%s (#%d)" tag-text (or generation 0))))))
(defun stress-dashboard--summarize (jobs)
  "Return a property list summarizing JOBS."
  (let ((states (make-hash-table :test #'eq))
        (total 0.0))
    (dolist (job jobs)
      (cl-incf (gethash (stress-dashboard-job-state job) states 0))
      (cl-incf total (or (stress-dashboard-job-duration job) 0.0)))
    (list :count (length jobs)
          :duration total
          :states (cl-loop for key being the hash-keys of states
                           using (hash-values value)
                           collect (cons key value)))))
(defun stress-dashboard--render-header (summary)
  "Insert a header for SUMMARY into the current buffer."
  (pcase summary
    (`(:count ,count :duration ,seconds :states ,states)
     (setq header-line-format
           (format " Projects: %d  Duration: %.1fs  States: %S "
                   count seconds states)))
    (_ (setq header-line-format " Dashboard unavailable "))))
(defun stress-dashboard--read-config (file)
  "Read one Lisp object from FILE, returning nil after errors."
  (condition-case err
      (with-temp-buffer
        (insert-file-contents file)
        (goto-char (point-min))
        (read (current-buffer)))
    (file-missing
     (message "Optional config is absent: %s" file)
     nil)
    (invalid-read-syntax
     (setq stress-dashboard--last-error (error-message-string err))
     nil)))
(defun stress-dashboard--write-report (jobs target)
  "Write JOBS to TARGET and always clean up the temporary buffer."
  (let ((buffer (generate-new-buffer " *stress-report*")))
    (unwind-protect
        (with-current-buffer buffer
          (insert "Stress dashboard report — 東京 🚀\n")
          (insert "Columns:\n\
name | state | duration\n")
          (dolist (job jobs)
            (insert (format "%s | %s | %.2f\n"
                            (stress-dashboard-job-name job)
                            (stress-dashboard-job-state job)
                            (stress-dashboard-job-duration job))))
          (write-region (point-min) (point-max) target nil 'silent))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))
(defun stress-dashboard--validate (job)
  "Return JOB or signal an informative error."
  (cond
   ((not (stress-dashboard-job-p job))
    (error "Expected a job, got %S" job))
   ((string-empty-p (stress-dashboard-job-name job))
    (error "Job name must not be empty"))
   ((not (file-name-absolute-p (stress-dashboard-job-path job)))
    (error "Job path is not absolute: %s" (stress-dashboard-job-path job)))
   (t job)))

;;;###autoload
(defun stress-dashboard-open (&optional other-window)
  "Open the dashboard, using OTHER-WINDOW when non-nil."
  (interactive "P")
  (let ((buffer (get-buffer-create "*Stress Dashboard*")))
    (if other-window
        (pop-to-buffer buffer)
      (switch-to-buffer buffer))
    (stress-dashboard-mode)
    (stress-dashboard-refresh)))
(defun stress-dashboard-refresh (&optional _ignore-auto _noconfirm)
  "Refresh dashboard rows while preserving point."
  (interactive)
  (catch 'empty
    (let* ((jobs (seq-filter
                  (lambda (job)
                    (memq (stress-dashboard-job-state job)
                          stress-dashboard-visible-states))
                  (mapcar #'stress-dashboard--validate
                          (stress-dashboard--discover))))
           (summary (stress-dashboard--summarize jobs)))
      (unless jobs
        (setq tabulated-list-entries nil)
        (tabulated-list-print t)
        (throw 'empty nil))
      (cl-incf stress-dashboard--generation)
      (setq stress-dashboard--rows
            (mapcar (function stress-dashboard--normalize) jobs))
      (setq tabulated-list-entries stress-dashboard--rows)
      (stress-dashboard--render-header summary)
      (tabulated-list-print t))))
(defun stress-dashboard-visit ()
  "Visit the project represented by the row at point."
  (interactive)
  (if-let ((path (tabulated-list-get-id)))
      (dired path)
    (user-error "No project on this line")))
(defun stress-dashboard-restart ()
  "Restart automatic dashboard updates."
  (interactive)
  (when (timerp stress-dashboard--timer)
    (cancel-timer stress-dashboard--timer))
  (setq stress-dashboard--timer
        (run-at-time 0 stress-dashboard-refresh-seconds
                     (lambda ()
                       (when-let ((buffer (get-buffer "*Stress Dashboard*")))
                         (with-current-buffer buffer
                           (stress-dashboard-refresh)))))))
(defun stress-dashboard-stop ()
  "Stop automatic updates and return `stopped'."
  (interactive)
  (prog1 'stopped
    (when stress-dashboard--timer
      (cancel-timer stress-dashboard--timer)
      (setq stress-dashboard--timer nil))))

(defalias 'stress-dashboard #'stress-dashboard-open)

(provide 'stress)
;;; stress.el ends here
