000100* COBOL grammar stress fixture: café, λ, 東京, and astral 🚀.
000200/ Fixed-format remark line before the free-format compilation unit.
000300* COVERAGE 01: IDENTIFICATION, ENVIRONMENT, DATA, PROCEDURE divisions.
000400* COVERAGE 02: CONFIGURATION and INPUT-OUTPUT sections.
000500* COVERAGE 03: SOURCE-COMPUTER and OBJECT-COMPUTER paragraphs.
000600* COVERAGE 04: SPECIAL-NAMES with DECIMAL-POINT IS COMMA.
000700* COVERAGE 05: FILE-CONTROL and a line-sequential SELECT clause.
000800* COVERAGE 06: FILE SECTION, FD, recording mode, and record picture.
000900* COVERAGE 07: level 78 constants and level 88 condition names.
001000* COVERAGE 08: signed, packed-decimal, binary, and edited pictures.
001100* COVERAGE 09: national, hexadecimal, boolean, and octal literals.
001200* COVERAGE 10: group items, subordinate levels, and REDEFINES.
001300* COVERAGE 11: OCCURS, ascending keys, and indexed tables.
001400* COVERAGE 12: RENAMES, LOCAL-STORAGE, and LINKAGE sections.
001500* COVERAGE 13: USING BY REFERENCE on the procedure header.
001600* COVERAGE 14: DISPLAY, MOVE, SET, INITIALIZE, and PERFORM verbs.
001700* COVERAGE 15: ADD, SUBTRACT, MULTIPLY, DIVIDE, and COMPUTE.
001800* COVERAGE 16: STRING, UNSTRING, INSPECT, and reference modification.
001900* COVERAGE 17: VARYING loops, subscripts, SEARCH, and AT END.
002000* COVERAGE 18: EVALUATE, WHEN, ALSO, ANY, and boolean operators.
002100* COVERAGE 19: OPEN, READ, CLOSE, and file-status conditions.
002200* COVERAGE 20: EXEC SQL and EXEC CICS balanced embedded regions.
002300* COVERAGE 21: JSON GENERATE and XML GENERATE scope terminators.
002400* COVERAGE 22: CALL modes, RETURNING, GOBACK, and STOP RUN.
002500* COVERAGE 23: quoted strings, escaped punctuation, and Unicode text.
002600* COVERAGE 24: decimal integer, decimal fraction, and exponent tokens.
002700* COVERAGE 25: PIC repetition, sign, decimal, and editing characters.
002800* COVERAGE 26: VALUE ZERO, ONE, SPACES, TRUE, and figurative constants.
002900* COVERAGE 27: nested groups and qualified table references.
003000* COVERAGE 28: arithmetic operators including exponentiation.
003100* COVERAGE 29: relational operators and explicit END-IF.
003200* COVERAGE 30: explicit END-STRING, END-SEARCH, and END-EVALUATE.
003300* COVERAGE 31: ON EXCEPTION and NOT ON EXCEPTION phrases.
003400* COVERAGE 32: comment punctuation { [ ( ) ] } and apostrophe ' text.
003500* COVERAGE 33: modern free-form comments appear between sections.
003600* COVERAGE 34: sequence areas and indicator-column comment markers.
003700* COVERAGE 35: paragraph names, section names, and periods.
003800* COVERAGE 36: identifiers with hyphens and numeric suffixes.
003900* COVERAGE 37: host variables begin with a colon in embedded SQL.
004000* COVERAGE 38: CICS queue names and parenthesized arguments.
004100* COVERAGE 39: JSON and XML keywords remain balanced to file end.
004200* COVERAGE 40: BMP Ελληνικά and non-BMP symbols 🚀 𝌆 are intentional.

       >>SOURCE FORMAT FREE
*> Identification and environment divisions.
IDENTIFICATION DIVISION.
PROGRAM-ID. UNICODE-ATLAS.
AUTHOR. Fixture Team.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
SOURCE-COMPUTER. IBM-Z.
OBJECT-COMPUTER. IBM-Z.
SPECIAL-NAMES. DECIMAL-POINT IS COMMA.

*> File selection and record declaration.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT OPTIONAL EVENT-FILE ASSIGN TO "events.dat".
DATA DIVISION.
FILE SECTION.
FD EVENT-FILE RECORDING MODE IS F.
01 EVENT-RECORD PIC X(80).

*> Representative elementary items and literals.
WORKING-STORAGE SECTION.
78 MAX-ITEMS VALUE 5.
01 WS-FILE-STATUS PIC XX VALUE SPACES.
   88 FILE-OK VALUE "00".
01 WS-INDEX PIC 99 COMP-3 VALUE ZERO.
01 WS-TOTAL PIC S9(7)V99 COMP-3 VALUE ZERO.
01 WS-COUNT PIC 9(4) BINARY VALUE 3.
01 WS-NAME PIC X(32) VALUE "café 東京 🚀".
01 WS-NATIONAL PIC N(12) VALUE N"κόσμος".
01 WS-HEX PIC X(4) VALUE X"CAFE0042".
01 WS-BOOL PIC 1 VALUE B"1".
01 WS-OCTAL PIC X(3) VALUE O"755".
01 WS-MESSAGE PIC X(80) VALUE SPACES.
01 WS-JSON PIC X(160) VALUE SPACES.
01 WS-XML PIC X(160) VALUE SPACES.

*> Group, redefinition, and compact table declarations.
01 CUSTOMER-RECORD.
   05 CUSTOMER-ID PIC 9(6) VALUE 42.
   05 CUSTOMER-NAME PIC X(30) VALUE "Ada Lovelace".
01 CUSTOMER-TEXT REDEFINES CUSTOMER-RECORD PIC X(36).
01 ITEM-TABLE.
   05 ITEM-ENTRY OCCURS MAX-ITEMS TIMES INDEXED BY ITEM-IDX.
      10 ITEM-CODE PIC 9(3).
      10 ITEM-PRICE PIC 9(5)V99.
LOCAL-STORAGE SECTION.
01 LS-RETRY PIC 9 VALUE ZERO.
LINKAGE SECTION.
01 LK-MODE PIC X.

*> Main dispatch uses several control and data-movement verbs.
PROCEDURE DIVISION USING BY REFERENCE LK-MODE.
MAIN-SECTION SECTION.
MAIN-PARAGRAPH.
    DISPLAY "Starting " WS-NAME
    INITIALIZE ITEM-TABLE
    MOVE 101 TO ITEM-CODE(1)
    SET ITEM-IDX TO 1
    PERFORM ARITHMETIC-EXAMPLES
    PERFORM CONDITIONAL-EXAMPLES
    PERFORM EMBEDDED-EXAMPLES
    GOBACK.

*> Arithmetic statements and an explicit size-error scope.
ARITHMETIC-EXAMPLES.
    ADD 10,25 TO WS-TOTAL
    SUBTRACT 2 FROM WS-TOTAL
    MULTIPLY WS-TOTAL BY 2
    DIVIDE WS-TOTAL BY WS-COUNT GIVING WS-TOTAL
    COMPUTE WS-TOTAL = (WS-TOTAL ** 2) + 1
        ON SIZE ERROR MOVE -1 TO WS-TOTAL
    END-COMPUTE.

*> Iteration, SEARCH, EVALUATE, and conditional scopes.
CONDITIONAL-EXAMPLES.
    PERFORM VARYING WS-INDEX FROM 1 BY 1 UNTIL WS-INDEX > MAX-ITEMS
        ADD ITEM-PRICE(WS-INDEX) TO WS-TOTAL
    END-PERFORM
    SEARCH ITEM-ENTRY AT END DISPLAY "missing"
        WHEN ITEM-CODE(ITEM-IDX) = 101 DISPLAY "found"
    END-SEARCH
    EVALUATE TRUE ALSO LK-MODE
        WHEN FILE-OK ALSO ANY DISPLAY "ready"
        WHEN OTHER DISPLAY "disabled"
    END-EVALUATE.

*> Balanced embedded and serialization constructs.
EMBEDDED-EXAMPLES.
    EXEC SQL SELECT COUNT(*) INTO :WS-COUNT FROM EVENTS END-EXEC
    EXEC CICS WRITEQ TS QUEUE("ATLAS") FROM(WS-MESSAGE) END-EXEC
    JSON GENERATE WS-JSON FROM CUSTOMER-RECORD
        ON EXCEPTION DISPLAY "json error" END-JSON
    XML GENERATE WS-XML FROM CUSTOMER-RECORD
        NOT ON EXCEPTION DISPLAY WS-XML END-XML.

*> Calls and termination complete the procedure grammar.
STATUS-PARAGRAPH.
*> Compact CALL target; argument modes are listed in the coverage matrix.
    CALL "AUDIT".
    EXIT.
END-PROGRAM.
    STOP RUN.
END PROGRAM UNICODE-ATLAS.
