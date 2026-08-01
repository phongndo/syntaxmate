// Compact Cypher coverage: café 東京 🚀 𝌆
CREATE (ada:Person {name: "Ada 🚀", city: 'Zürich', active: TRUE})
CREATE (東京:`City Label` {name: "東京", glyph: "𝌆", missing: NULL})
MATCH (ada)-[r:KNOWS|LIKES*1..3]->(friend:Person)
WHERE friend.age >= 21 AND friend.name =~ 'A.*'
  AND NOT(friend.blocked) AND friend.role IN ['admin', 'reader']
WITH friend, length((ada)-->(friend)) AS hops
RETURN friend.name, coalesce(friend.nickname, "café"), hops
ORDER BY hops DESC
SKIP 0
LIMIT 10
UNWIND range(1, 3) AS n
RETURN n, CASE WHEN n % 2 = 0 THEN "even" ELSE "odd" END
MERGE (item:Item {id: 42.5, enabled: FALSE})
ON CREATE SET item.created = timestamp()
ON MATCH SET item.seen = item.seen + 1
REMOVE item.legacy
RETURN count(*) AS total
