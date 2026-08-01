/*
 * Tier B SurrealQL stress fixture: hand-written, grammar-driven, and reviewable.
 * Unicode sentinels include BMP café λ 東京 雪 and astral 🚀 𝌆.
 */

-- Namespace, database, option, and transaction statements.
USE NS mark;
USE DB syntax_lab;
OPTION IMPORT;
BEGIN TRANSACTION;

# Schema declarations exercise keyword families and type-shaped values.
DEFINE TABLE person SCHEMAFULL
  CHANGEFEED 7d INCLUDE ORIGINAL
  PERMISSIONS FOR select FULL, FOR create, update WHERE $auth.admin = true;
DEFINE FIELD name ON TABLE person TYPE string
  ASSERT $value != NONE AND $value != "";
DEFINE FIELD age ON person TYPE option<number> DEFAULT NONE
  ASSERT $value = NONE OR $value >= 0;
DEFINE FIELD active ON person TYPE bool DEFAULT true;
DEFINE FIELD tags ON person TYPE array<string> DEFAULT [];
DEFINE FIELD address ON person FLEXIBLE TYPE object;
DEFINE FIELD address.city ON person TYPE string VALUE $value;
DEFINE FIELD friends ON person TYPE array<record<person>> REFERENCE;
DEFINE FIELD created_at ON person TYPE datetime DEFAULT d"2026-07-12" READONLY;
DEFINE FIELD updated_at ON person TYPE datetime VALUE d"2026-07-12";
DEFINE INDEX person_name ON person FIELDS name UNIQUE;
DEFINE INDEX person_search ON person FIELDS name SEARCH ANALYZER simple BM25 HIGHLIGHTS;

DEFINE TABLE follows TYPE RELATION IN person OUT person ENFORCED;
DEFINE FIELD since ON follows TYPE datetime DEFAULT d"2020-01-01";
DEFINE FIELD weight ON follows TYPE number DEFAULT 1.0;
DEFINE INDEX follows_pair ON follows FIELDS in, out UNIQUE;

DEFINE PARAM $application_name VALUE "Mark café 🚀" PERMISSIONS FULL;
DEFINE ANALYZER simple TOKENIZERS blank, class FILTERS lowercase, ascii;
DEFINE EVENT person_created ON TABLE person
  WHEN $event = "CREATE"
  THEN (CREATE audit CONTENT { action: $event, target: $after.id });
DEFINE FUNCTION fn::display
  ($name: string, $city: option<string>) {
  RETURN $name + " @ " + ($city ?? "unknown");
} PERMISSIONS FULL;

-- Scalars, prefixed strings, durations, booleans, voids, and identifiers.
LET $integer = 42;
LET $decimal = 3.14159f;
LET $precise = 19.95dec;
LET $duration = 1w2d3h4m5s6ms7µs8ns;
LET $moment = d"2026-07-12T09:30:15Z";
LET $uuid = u"01234567-89ab-cdef-0123-456789abcdef";
LET $regex = /(?i)caf[eé]/;
LET $truth = TRUE;
LET $lie = False;
LET $missing = NULL;
LET $nothing = none;
LET $`quoted_var` = "backtick variable";
LET $⟨angle_var⟩ = 'angle variable';
LET $labels = [`display-name`, ⟨東京 label🚀⟩, "雪", '𝌆'];

-- Nested arrays and objects keep every multiline begin/end state closed.
LET $profile = {
  name: "Ada Lovelace",
  aliases: ['Ada', "Enchantress of Numbers", "café"],
  location: {
    city: "Zürich",
    coordinates: [47.3769, 8.5417],
    unicode: { bmp: "東京 雪 λ", astral: "🚀 𝌆" }
  },
  flags: [true, FALSE, null, NONE],
  durations: [250ms, 3s, 2h30m],
  note: "escaped quote: \" with a closed delimiter"
};

-- Record-id spellings covered by the grammar.
LET $ada = person:ada;
LET $grace = person:`grace-hopper`;
LET $tokyo = person:⟨東京-user🚀⟩;
LET $generated = person:⟨generated-id⟩;
LET $compound = metric:["eu-west", 2026, true];
LET $object_id = sample:{ region: "日本", sequence: 7 };

CREATE person:ada CONTENT $profile RETURN AFTER;
CREATE person:`grace-hopper` SET
  name = 'Grace Hopper', age = 85, active = true,
  tags = ["compiler", "navy", "𝌆"], address.city = "New York"
RETURN AFTER;
CREATE person:⟨東京-user🚀⟩ CONTENT {
  name: "東京 User 🚀",
  age: 31,
  active: TRUE,
  tags: ["日本", "café", "space"],
  address: { city: "東京", glyph: "𝌆" }
};

INSERT INTO person (id, name, age, active) VALUES
  (person:linus, "Linus", 56, true),
  (person:margaret, "Margaret", 89, FALSE)
ON DUPLICATE KEY UPDATE active = true RETURN AFTER;

UPSERT person:ada MERGE { active: true, visits: 1 } RETURN DIFF;
UPDATE person:ada SET
  age += 1,
  visits = (visits ?? 0) + 1,
  tags += "updated",
  obsolete -= "legacy"
RETURN BEFORE;
UPDATE person SET active = false WHERE age > 120 TIMEOUT 2s PARALLEL;

RELATE person:ada->follows->person:`grace-hopper`
  CONTENT { since: d"2020-01-01", weight: 0.9 } RETURN AFTER;
RELATE person:⟨東京-user🚀⟩->follows->person:ada
  SET since = d"2026-07-12", weight = 1.25;

-- Projection, aliases, aggregation, grouping, ordering, paging, and fetch.
SELECT
  id,
  name AS display_name,
  name + " @ " + (address.city ?? "unknown") AS label,
  age * 1.5 + 2 / 3 AS adjusted,
  tags[0] AS first_tag,
  address.city ?? "unknown" AS city
FROM person
WHERE active IS true
  AND age >= 18
  AND age <= 100
  AND name != "nobody"
ORDER BY age DESC, name COLLATE NUMERIC ASC
LIMIT 25 START 0
FETCH friends;

SELECT count AS total, age AS average_age
FROM person
WHERE tags CONTAINSANY ["café", "compiler"]
GROUP ALL;

SELECT * FROM person
WHERE id INSIDE [person:ada, person:`grace-hopper`]
  OR tags CONTAINS "space"
EXPLAIN FULL;

-- Containment, set, fuzzy, comparison, logical, and arithmetic operators.
RETURN [1, 2, 3] CONTAINSALL [1, 2];
RETURN [1, 2, 3] CONTAINSANY [3, 4];
RETURN [1, 2, 3] CONTAINSNONE [8, 9];
RETURN [1, 2, 3] CONTAINSSOME [2, 9];
RETURN [1, 2, 3] CONTAINSNOT 7;
RETURN 2 INSIDE [1, 2, 3];
RETURN 7 NOTINSIDE [1, 2, 3];
RETURN [1, 2] ALLINSIDE [1, 2, 3];
RETURN [2, 8] ANYINSIDE [1, 2, 3];
RETURN [8, 9] NONEINSIDE [1, 2, 3];
RETURN [2, 9] SOMEINSIDE [1, 2, 3];
RETURN { type: "Point", coordinates: [1, 2] } INTERSECTS { type: "Point", coordinates: [1, 2] };
RETURN "café" ~ "cafe" AND "東京" !~ "大阪";
RETURN ["a", "b"] *= "a" OR ["x", "b"] ?= "b";
RETURN ["Ada"] *~ "ada" && ["Grace"] ?~ "grace";
RETURN 2 × 3 + 8 ÷ 4 - 1;
RETURN $profile.name ?: "anonymous";
RETURN $nothing ?? "fallback";
RETURN $profile.embedding <|10,COSINE|> $profile.embedding;
RETURN name @2@ "surreal database";

-- Symbolic Unicode containment operators are valid SurrealQL alternatives.
RETURN [1, 2, 3] ⊇ [1, 2];
RETURN [1, 2, 3] ⊃ [3, 8];
RETURN [1, 2, 3] ⊅ [8, 9];
RETURN [1, 2, 3] ∋ 2;
RETURN [1, 2, 3] ∌ 9;
RETURN 2 ∈ [1, 2, 3];
RETURN 9 ∉ [1, 2, 3];
RETURN [1, 2] ⊆ [1, 2, 3];
RETURN [2, 8] ⊂ [1, 2, 3];
RETURN [8, 9] ⊄ [1, 2, 3];

-- Subqueries, graph navigation, ranges, destructuring, and control flow.
LET $adults = (SELECT id, name FROM person WHERE age >= 18);
SELECT name, ->follows->person.name AS following FROM person:ada;
SELECT <-follows<-person.name AS followers FROM person:`grace-hopper`;
SELECT <->follows<->person AS neighbours FROM person:ada;
SELECT * FROM person WHERE 18 <~ age;

FOR $person IN (SELECT * FROM person) {
  IF $person.active = true {
    CONTINUE;
  } ELSE {
    UPDATE $person.id SET reviewed = true;
  };
};

-- Function values exist in SurrealQL; calls are omitted to keep this parity fixture deterministic.
LET $formatter = { label: "value", unicode: "café 東京 🚀 𝌆", ok: true };

LIVE SELECT * FROM person WHERE active = true;
KILL $live_query_id;
SHOW CHANGES FOR TABLE person SINCE 1h LIMIT 20;
INFO FOR TABLE person STRUCTURE;
SLEEP 10ms;

DELETE follows WHERE weight < 0 RETURN BEFORE;
REMOVE INDEX person_search ON TABLE person;
REMOVE EVENT person_created ON TABLE person;

COMMIT TRANSACTION;
RETURN { status: "done", generated: $generated, labels: $labels };

// Final slash comment closes at newline: brackets [] braces {} arrows -> <-.
