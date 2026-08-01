C BASIC FIXED-FORM SAMPLE: COMMENTS MAY CONTAIN λ, 雪, AND 🚀.
* ASTERISK IN COLUMN ONE IS ALSO A FULL-LINE COMMENT.
      PROGRAM BASIC
      IMPLICIT NONE
      INTEGER I
      REAL VALUES(4), TOTAL
      LOGICAL RISING
      CHARACTER*80 GREETING
      DATA VALUES /1.0, 2.5, 4.0, 8.0/
      GREETING = 'Hello, λ雪 and 🚀 in fixed form; this quoted text
     1 continues on another physical line.'
      TOTAL = 0.0
      RISING = .TRUE.
      WRITE (*,100) GREETING
      DO 20 I = 1, 4
      IF (RISING .AND. VALUES(I) .GE. 2.0) THEN
      TOTAL = TOTAL + VALUES(I)
      ELSE
      CYCLE
      END IF
   20 CONTINUE
      WRITE (*,110) TOTAL
  100 FORMAT (1X,A,
     1        /,1X,'BEGIN VALUES')
  110 FORMAT (1X,'TOTAL=',F8.2) ! INLINE COMMENT AFTER A STATEMENT
      STOP
      END
