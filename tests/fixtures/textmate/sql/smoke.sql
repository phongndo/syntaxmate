-- SQL smoke fixture: café λ
SELECT id, name
FROM users
WHERE active = TRUE
ORDER BY name;
