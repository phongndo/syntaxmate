       IDENTIFICATION DIVISION.
       PROGRAM-ID. BASIC-FIXTURE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-GREETING PIC X(40) VALUE "café 東京 🚀".
       01  WS-TOTAL    PIC 9(4) VALUE ZERO.
       01  WS-INDEX    PIC 9(2) VALUE ONE.
       PROCEDURE DIVISION.
       MAIN-PARAGRAPH.
           *> Unicode includes BMP text and the astral rocket 🚀.
           PERFORM VARYING WS-INDEX FROM 1 BY 1
               UNTIL WS-INDEX > 3
               ADD WS-INDEX TO WS-TOTAL
           END-PERFORM
           IF WS-TOTAL GREATER THAN ZERO
               DISPLAY WS-GREETING ": " WS-TOTAL
           ELSE
               DISPLAY "no values"
           END-IF
           STOP RUN.
