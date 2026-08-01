-- SQL basic fixture: café, 東京, λ, 🚀, and astral 𝌆.
-- Strings, constraints, numbers, functions, and relational clauses.
BEGIN TRANSACTION;
CREATE TABLE observatory (
    id BIGINT PRIMARY KEY,
    name VARCHAR(80) NOT NULL,
    payload TEXT DEFAULT '東京 λ',
    score DECIMAL(8, 2) CHECK (score >= 0),
    active BOOLEAN DEFAULT TRUE
);

INSERT INTO observatory (id, name, payload, score, active) VALUES
    (1, 'café ''A'' 🚀 𝌆', 'ready', 42.50, TRUE),
    (2, 'night lab', NULL, 7.25, FALSE);

SELECT o.id,
       UPPER(o.name) AS display_name,
       COALESCE(o.payload, 'none') AS payload,
       COUNT(*) AS samples
FROM observatory AS o
WHERE o.active = TRUE AND o.name LIKE '%café%'
GROUP BY o.id, o.name, o.payload
HAVING COUNT(*) >= 1
ORDER BY o.score DESC;
COMMIT;
