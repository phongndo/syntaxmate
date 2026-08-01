REPORT z_textmate_basic.
* Full-line comment: café λ 東京 🚀 𝌆
CLASS lcl_greeter DEFINITION FINAL.
  PUBLIC SECTION.
    METHODS greet IMPORTING iv_name TYPE string RETURNING VALUE(rv_text) TYPE string.
ENDCLASS.

CLASS lcl_greeter IMPLEMENTATION.
  METHOD greet.
    DATA(lv_count) = 2.
    DATA(lv_quote) = 'l''été'.
    rv_text = |Hello { iv_name CASE = UPPER }, café λ 東京 🚀 𝌆|.
    DATA(lv_multiline) = |first lexical line
second lexical line: 東京 🚀 𝌆|.
    IF lv_count >= 2 AND sy-subrc = 0.
      rv_text = rv_text && ` ready`.
    ELSE.
      CLEAR rv_text.
    ENDIF.
    " Partial-line comment with Unicode λ
  ENDMETHOD.
ENDCLASS.
