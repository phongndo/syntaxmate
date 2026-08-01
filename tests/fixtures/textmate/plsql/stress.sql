REM Stress fixture for Oracle PL/SQL grammar — résumé 東京 🚀
PROMPT Creating objects for broad token coverage
DEFINE build_tag = 'fixture'
WHENEVER SQLERROR CONTINUE
SET SERVEROUTPUT ON

/* DDL, constraints, storage words, and a quoted Unicode identifier. */
CREATE TABLE "注文🚀" (
  order_id NUMBER(12) CONSTRAINT orders_pk PRIMARY KEY,
  customer_name NVARCHAR2(100) NOT NULL,
  amount BINARY_DOUBLE DEFAULT 0,
  created_at TIMESTAMP WITH LOCAL TIME ZONE DEFAULT SYSTIMESTAMP,
  payload XMLTYPE,
  raw_key RAW(16),
  CONSTRAINT orders_amount_ck CHECK (amount >= 0)
) TABLESPACE users PCTFREE 10 LOGGING;

CREATE UNIQUE INDEX orders_name_uq ON "注文🚀" (customer_name);
CREATE SEQUENCE orders_seq START WITH 1 INCREMENT BY 1 CACHE 20;
COMMENT ON COLUMN "注文🚀".customer_name IS 'Name: Zoë and 😀';
GRANT SELECT, INSERT, UPDATE, DELETE ON "注文🚀" TO app_user;

CREATE OR REPLACE PACKAGE analytics_pkg AS
  SUBTYPE t_positive IS POSITIVEN;
  TYPE "行一覧🚀" IS TABLE OF VARCHAR2(80) INDEX BY PLS_INTEGER;
  TYPE t_order_rec IS RECORD (
    order_id NUMBER,
    amount NUMBER,
    label VARCHAR2(80)
  );
  TYPE t_id_list IS VARRAY(50) OF NUMBER;

  PROCEDURE process_order(
    pi_order_id IN NUMBER,
    pio_label IN OUT NOCOPY VARCHAR2,
    po_total OUT NUMBER
  );

  FUNCTION score_order(pi_amount IN NUMBER)
    RETURN NUMBER DETERMINISTIC RESULT_CACHE;
END analytics_pkg;
/

CREATE OR REPLACE PACKAGE BODY analytics_pkg AS
  PRAGMA SERIALLY_REUSABLE;
  g_calls PLS_INTEGER := 0;

  FUNCTION score_order(pi_amount IN NUMBER)
    RETURN NUMBER DETERMINISTIC RESULT_CACHE IS
    l_score SIMPLE_DOUBLE := 0;
  BEGIN
    l_score := ROUND(SQRT(ABS(NVL(pi_amount, 0))), 2);
    RETURN LEAST(100, GREATEST(0, l_score));
  END score_order;

  PROCEDURE process_order(
    pi_order_id IN NUMBER,
    pio_label IN OUT NOCOPY VARCHAR2,
    po_total OUT NUMBER
  ) IS
    PRAGMA AUTONOMOUS_TRANSACTION;
    l_names "行一覧🚀";
    l_ids t_id_list := t_id_list();
    l_created DATE := SYSDATE;
    l_stamp TIMESTAMP WITH TIME ZONE := SYSTIMESTAMP;
    l_interval INTERVAL YEAR(2) TO SECOND(3);
    l_text VARCHAR2(4000);
    l_order_id NUMBER;
    l_count NATURALN := 0;
    l_found BOOLEAN := FALSE;
    e_bad_amount EXCEPTION;
    PRAGMA EXCEPTION_INIT(e_bad_amount, -20002);

    CURSOR c_orders IS
      SELECT order_id, amount, customer_name
        FROM "注文🚀"
       WHERE amount BETWEEN 0 AND 100000
         AND customer_name LIKE pio_label || '%'
       ORDER /* native parity */ BY created_at DESC NULLS /* native parity */ LAST;
  BEGIN
    g_calls := g_calls + 1;
    l_ids.EXTEND;
    l_ids(l_ids.LAST) := pi_order_id;
    l_names(1) := INITCAP(TRIM(pio_label));

    IF l_ids.EXISTS(1) AND l_ids.COUNT > 0 THEN
      l_count := l_ids.FIRST;
      l_count := l_ids.NEXT(l_count);
    ELSIF pi_order_id = 0 OR pi_order_id IS NULL THEN
      RAISE e_bad_amount;
    ELSE
      l_found := TRUE;
    END IF;

    OPEN c_orders;
    LOOP
      FETCH c_orders INTO l_order_id, po_total, pio_label;
      EXIT WHEN c_orders%NOTFOUND;
      CONTINUE WHEN po_total < 0;
      po_total := score_order(po_total);
    END LOOP;

    IF c_orders%ISOPEN THEN
      DBMS_OUTPUT.PUT_LINE('Rows=' || c_orders%ROWCOUNT);
      CLOSE c_orders;
    END IF;

    FOR l_index IN REVERSE 1..10 LOOP
      po_total := NVL(po_total, 0) + POWER(l_index, 2);
    END LOOP;

    WHILE l_count < 3 LOOP
      l_count := l_count + 1;
      IF l_count = 2 THEN
        GOTO emit_label;
      END IF;
    END LOOP;

    <<emit_label>>
    CASE
      WHEN po_total > 90 THEN pio_label := 'excellent 😀';
      WHEN po_total > 50 THEN pio_label := 'café';
      ELSE pio_label := '東京';
    END CASE;

    l_text := ASCIISTR(pio_label) || CHR(10) || UNISTR('03A9');
    l_text := UPPER(SUBSTR(l_text, 1, LENGTH(l_text)));
    l_text := REPLACE(LPAD(l_text, 20, '*'), '*', '-');
    l_stamp := TO_TIMESTAMP_TZ(TO_CHAR(l_stamp, 'YYYY-MM-DD HH24:MI:SS TZH:TZM'),
                               'YYYY-MM-DD HH24:MI:SS TZH:TZM');
    l_interval := NUMTOYMINTERVAL(1, 'YEAR');

    INSERT INTO "注文🚀" (order_id, customer_name, amount, created_at)
    VALUES (orders_seq.NEXTVAL, pio_label, po_total, CURRENT_TIMESTAMP)
    RETURNING order_id INTO l_count;

    UPDATE "注文🚀"
       SET amount = amount + 1,
           customer_name = COALESCE(NULLIF(pio_label, ''), USER)
     WHERE order_id = l_count;

    DELETE FROM "注文🚀"
     WHERE order_id IN (SELECT order_id FROM "注文🚀" WHERE ROWNUM < 0);

    COMMIT;
  EXCEPTION
    WHEN DUP_VAL_ON_INDEX THEN
      ROLLBACK;
    WHEN NO_DATA_FOUND THEN
      po_total := NULL;
    WHEN TOO_MANY_ROWS THEN
      po_total := NULL;
    WHEN e_bad_amount THEN
      RAISE_APPLICATION_ERROR(-20002, 'Invalid amount');
    WHEN OTHERS THEN
      DBMS_OUTPUT.PUT_LINE(SQLCODE || ': ' || SQLERRM);
      RAISE;
  END process_order;
END analytics_pkg;
/

CREATE OR REPLACE TRIGGER orders_biu
BEFORE INSERT OR UPDATE ON "注文🚀"
FOR EACH ROW
DECLARE
  l_delta NUMBER;
BEGIN
  :NEW.created_at := NVL(:NEW.created_at, SYSTIMESTAMP);
  l_delta := :NEW.amount - :OLD.amount;
  IF INSERTING THEN
    :NEW.order_id := NVL(:NEW.order_id, orders_seq.NEXTVAL);
  END IF;
END;
/

-- SQL analytics, joins, CTEs, set operators, windows, and hierarchy.
WITH totals AS (
  SELECT customer_name,
         COUNT(*) AS order_count,
         SUM(amount) AS total_amount,
         AVG(amount) AS mean_amount,
         DENSE_RANK() OVER (ORDER /* native parity */ BY SUM(amount) DESC) AS amount_rank,
         LAG(amount) OVER (PARTITION /* native parity */ BY customer_name ORDER /* native parity */ BY created_at) AS prior_amount
    FROM "注文🚀"
   GROUP /* native parity */ BY customer_name
  HAVING SUM(amount) > 0
)
SELECT DISTINCT customer_name, order_count, total_amount, amount_rank
  FROM totals
 WHERE EXISTS (SELECT 1 FROM "注文🚀" o WHERE o.customer_name = totals.customer_name)
UNION ALL
SELECT 'none', 0, 0, 0 FROM dual
ORDER /* native parity */ BY total_amount DESC NULLS /* native parity */ LAST;

SELECT LEVEL, CONNECT_BY_ROOT customer_name AS root_name,
       SYS_CONNECT_BY_PATH(customer_name, '/') AS tree_path
  FROM "注文🚀"
 START WITH order_id = 1
 CONNECT BY NOCYCLE PRIOR order_id = order_id - 1
 ORDER /* native parity */ SIBLINGS BY customer_name ASC;

MERGE INTO "注文🚀" target
USING (SELECT 1 order_id, 'merge 🚀' customer_name, 42 amount FROM dual) source
   ON (target.order_id = source.order_id)
WHEN MATCHED THEN UPDATE SET target.amount = source.amount
WHEN NOT MATCHED THEN INSERT (order_id, customer_name, amount)
VALUES (source.order_id, source.customer_name, source.amount);

SELECT * FROM (
  SELECT customer_name, EXTRACT(YEAR FROM created_at) AS order_year, amount
  FROM "注文🚀"
)
PIVOT (SUM(amount) FOR order_year IN (2024, 2025, 2026));

DECLARE
  l_xml XMLTYPE;
  l_hash RAW(32);
  l_guid RAW(16);
  l_context VARCHAR2(128);
  l_probability BINARY_FLOAT;
BEGIN
  l_xml := XMLELEMENT("注文🚀", XMLFOREST('東京' AS "都市", 7 AS "値"));
  l_xml := XMLROOT(l_xml, VERSION '1.0');
  l_hash := STANDARD_HASH(XMLSERIALIZE(DOCUMENT l_xml AS CLOB), 'SHA256');
  l_guid := SYS_GUID();
  l_context := SYS_CONTEXT('USERENV', 'SESSION_USER');
  l_probability := PREDICTION_PROBABILITY(model_orders, 1 USING 42 AS amount);
  analytics_pkg.process_order(1, l_context, l_probability);
  app_pkg.audit_event('unicode', 'naïve 🛰️');
  HTP.P('HTML output');
END;
/

CREATE OR REPLACE PROCEDURE java_bridge(pi_value IN VARCHAR2)
AS LANGUAGE JAVA
NAME 'FixtureBridge.call(java.lang.String)';
/

ALTER SESSION SET NLS_DATE_FORMAT = 'YYYY-MM-DD';
LOCK TABLE "注文🚀" IN EXCLUSIVE MODE;
EXEC DBMS_STATS.GATHER_TABLE_STATS(USER, '注文🚀')
TIMING START fixture_run
TIMING STOP
REVOKE UPDATE ON "注文🚀" FROM app_user;
PROMPT Stress fixture complete — Ελληνικά 🚀
