-- Compact SurrealQL grammar coverage: café 東京 🚀 𝌆
USE NS mark DB fixtures;
DEFINE TABLE person SCHEMAFULL PERMISSIONS FULL;
DEFINE FIELD name ON person TYPE string ASSERT $value != NONE;
DEFINE FIELD tags ON person TYPE array<string> DEFAULT [];
DEFINE INDEX person_name ON person FIELDS name UNIQUE;
LET $greeting = "Bonjour, café 🚀";
LET $profile = {
  name: 'Ada', active: true, score: 42.5dec,
  places: ["Zürich", "東京", "𝌆"], delay: 1h30m
};
CREATE person:ada CONTENT $profile;
CREATE person:⟨東京-user🚀⟩ SET name = "東京", active = FALSE;
SELECT name, tags, score AS total FROM person
  WHERE active IS true AND score >= 21
  ORDER BY name COLLATE ASC LIMIT 10 START 0;
UPDATE person:ada SET score += 1, tags += "grammar" RETURN AFTER;
RELATE person:ada->follows->person:`grace-hopper` CONTENT { since: d"2026-07-12" };
RETURN ($greeting + " / " + $profile.name) ?? "none";
/* Closed block comment with [] {} arrows -> and Unicode 雪. */
