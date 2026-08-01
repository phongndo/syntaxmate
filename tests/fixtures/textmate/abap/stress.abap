REPORT z_textmate_stress MESSAGE-ID 00.
* Grammar stress fixture: café λ 東京 🚀 𝌆
* Exercises declarations, templates, operators, control flow, SQL, and names.

TYPES: BEGIN OF ty_item,
         id       TYPE i,
         name     TYPE string,
         quantity TYPE int8,
         price    TYPE decfloat34,
       END OF ty_item.
TYPES ty_items TYPE SORTED TABLE OF ty_item WITH UNIQUE KEY id.

CONSTANTS gc_limit TYPE i VALUE 10.
DATA gt_items TYPE ty_items.
DATA gv_title TYPE string VALUE 'café λ 東京 🚀 𝌆'.
FIELD-SYMBOLS <item> TYPE ty_item.

INTERFACE lif_renderable.
  METHODS render RETURNING VALUE(rv_text) TYPE string.
ENDINTERFACE.

CLASS lcl_catalog DEFINITION FINAL.
  PUBLIC SECTION.
    INTERFACES lif_renderable.
    METHODS constructor IMPORTING iv_title TYPE string DEFAULT `Catalog`.
    METHODS add
      IMPORTING
        iv_id       TYPE i
        iv_name     TYPE string
        iv_quantity TYPE int8
        iv_price    TYPE decfloat34
      RAISING cx_sy_itab_duplicate_key.
    METHODS find_name
      IMPORTING iv_pattern TYPE string
      RETURNING VALUE(rv_name) TYPE string.
    CLASS-METHODS normalize
      IMPORTING iv_text TYPE string
      RETURNING VALUE(rv_text) TYPE string.
  PRIVATE SECTION.
    DATA mv_title TYPE string.
    DATA mt_items TYPE ty_items.
ENDCLASS.

CLASS lcl_catalog IMPLEMENTATION.
  METHOD constructor.
    mv_title = iv_title.
  ENDMETHOD.

  METHOD add.
    INSERT VALUE #(
      id       = iv_id
      name     = iv_name
      quantity = iv_quantity
      price    = iv_price )
      INTO TABLE mt_items.
  ENDMETHOD.

  METHOD normalize.
    rv_text = to_upper( condense( iv_text ) ).
    REPLACE ALL OCCURRENCES OF REGEX `\s+` IN rv_text WITH ` `.
  ENDMETHOD.

  METHOD find_name.
    DATA(lv_pattern) = to_lower( iv_pattern ).
    LOOP AT mt_items ASSIGNING FIELD-SYMBOL(<row>).
      IF to_lower( <row>-name ) CP |*{ lv_pattern }*|.
        rv_name = <row>-name.
        EXIT.
      ENDIF.
    ENDLOOP.
  ENDMETHOD.

  METHOD lif_renderable~render.
    DATA(lv_total) = CONV decfloat34( 0 ).
    DATA(lv_lines) = ``.
    DATA(lv_header) = |{ mv_title CASE = UPPER WIDTH = 24 ALIGN = CENTER }|.
    LOOP AT mt_items ASSIGNING <item>.
      lv_total = lv_total + <item>-price * <item>-quantity.
      lv_lines = lv_lines &&
        |{ <item>-id WIDTH = 3 }: { <item>-name CASE = UPPER } | &&
        |x { <item>-quantity } = { <item>-price DECIMALS = 2 }\n|.
    ENDLOOP.
    rv_text = |{ lv_header }\n{ lv_lines }Total: { lv_total DECIMALS = 2 }|.
  ENDMETHOD.
ENDCLASS.

CLASS ltc_catalog DEFINITION FINAL FOR TESTING
  RISK LEVEL HARMLESS DURATION SHORT.
  PRIVATE SECTION.
    METHODS setup.
    METHODS renders_unicode FOR TESTING.
    METHODS handles_control_flow FOR TESTING.
    DATA mo_cut TYPE REF TO lcl_catalog.
ENDCLASS.

CLASS ltc_catalog IMPLEMENTATION.
  METHOD setup.
    mo_cut = NEW #( |café λ 東京 🚀 𝌆| ).
  ENDMETHOD.

  METHOD renders_unicode.
    TRY.
        mo_cut->add(
          iv_id       = 1
          iv_name     = 'Crème brûlée'
          iv_quantity = 2
          iv_price    = '3.50' ).
        mo_cut->add(
          iv_id       = 2
          iv_name     = '東京 rocket 🚀'
          iv_quantity = 1
          iv_price    = '9.75' ).
      CATCH cx_sy_itab_duplicate_key INTO DATA(lx_duplicate).
        DATA(lv_error) = lx_duplicate->get_text( ).
        MESSAGE lv_error TYPE 'E'.
    ENDTRY.

    DATA(lv_output) = mo_cut->lif_renderable~render( ).
    cl_abap_unit_assert=>assert_true(
      act = xsdbool( contains( val = lv_output sub = '東京' ) ) ).
    cl_abap_unit_assert=>assert_not_initial( lv_output ).
  ENDMETHOD.

  METHOD handles_control_flow.
    DATA(lv_index) = 0.
    DATA(lv_state) = abap_undefined.
    WHILE lv_index < gc_limit.
      lv_index += 1.
      CASE lv_index MOD 3.
        WHEN 0.
          lv_state = abap_true.
        WHEN 1 OR 2.
          lv_state = abap_false.
        WHEN OTHERS.
          CONTINUE.
      ENDCASE.
      IF lv_index >= 8.
        EXIT.
      ELSEIF lv_index = 4.
        CHECK lv_state IS NOT INITIAL.
      ELSE.
        ASSERT lv_index > 0.
      ENDIF.
    ENDWHILE.
    cl_abap_unit_assert=>assert_equals( act = lv_index exp = 8 ).
  ENDMETHOD.
ENDCLASS.

START-OF-SELECTION.
  DATA(lo_catalog) = NEW lcl_catalog( gv_title ).
  DO 3 TIMES.
    TRY.
        lo_catalog->add(
          iv_id       = sy-index
          iv_name     = SWITCH string( sy-index
            WHEN 1 THEN `alpha`
            WHEN 2 THEN `café`
            ELSE |東京 🚀 𝌆| )
          iv_quantity = sy-index * 2
          iv_price    = CONV decfloat34( sy-index ) / 3 ).
      CATCH cx_sy_itab_duplicate_key.
        CONTINUE.
    ENDTRY.
  ENDDO.

  DATA(lv_report) = lo_catalog->lif_renderable~render( ).
  DATA(lv_escaped_pipe) = |left \| center \| right|.
  DATA(lv_multiline) = |first template line: café λ
second template line: 東京 🚀
third template line: astral 𝌆|.
  WRITE: / text-001, lv_report,
         / lv_escaped_pipe,
         / lv_multiline.

  IF sy-subrc EQ 0 AND sy-uname IS NOT INITIAL.
    WRITE / |User { sy-uname } at { sy-uzeit TIME = USER }|. ##NO_TEXT
  ENDIF.

FORM summarize USING it_items TYPE ty_items
               CHANGING cv_sum TYPE decfloat34.
  CLEAR cv_sum.
  LOOP AT it_items ASSIGNING FIELD-SYMBOL(<summary_item>).
    cv_sum = cv_sum + <summary_item>-price * <summary_item>-quantity.
  ENDLOOP.
ENDFORM.

SELECT carrid,
       connid,
       SUM( seatsocc ) AS occupied
  FROM sflight
  WHERE carrid <> @space
  GROUP BY carrid, connid
  ORDER BY carrid, connid
  INTO TABLE @DATA(lt_flights)
  UP TO 5 ROWS.

LOOP AT lt_flights INTO DATA(ls_flight).
  WRITE: / ls_flight-carrid, ls_flight-connid, ls_flight-occupied.
ENDLOOP.
