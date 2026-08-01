-- Basic Oracle PL/SQL: café, 東京, and 🚀 are safe Unicode text.
CREATE OR REPLACE PROCEDURE "greet_user" (
  pio_name IN OUT NOCOPY VARCHAR2,
  pi_count IN PLS_INTEGER DEFAULT 1
) IS
  l_message VARCHAR2(200) := 'Hello, 世界 🌍';
  l_total NUMBER := 0;
BEGIN
  /* Exercise control flow and arithmetic operators. */
  IF pi_count > 0 AND pio_name IS NOT NULL THEN
    FOR l_index IN 1..pi_count LOOP
      l_total := l_total + l_index;
      DBMS_OUTPUT.PUT_LINE(l_message || ': ' || pio_name);
    END LOOP;
  ELSE
    pio_name := 'anonymous';
  END IF;
  SELECT UPPER(pio_name) INTO pio_name FROM dual;
  COMMIT;
EXCEPTION
  WHEN VALUE_ERROR THEN ROLLBACK;
  WHEN OTHERS THEN RAISE_APPLICATION_ERROR(-20001, SQLERRM);
END greet_user;
/
PROMPT Finished basic PL/SQL fixture — Καλημέρα 🚀
