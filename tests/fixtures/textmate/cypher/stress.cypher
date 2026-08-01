// Cypher grammar stress fixture: café λ 東京 雪 🚀 𝌆
// Comments exercise punctuation ()[]{} -> <- -- and escaped-looking \n text.

CREATE (ada:Person {name: 'Ada', age: 36, active: TRUE, note: "café 🚀"});
CREATE (grace:Person {name: "Grace", age: 44, active: FALSE, note: 'compiler 𝌆'});
CREATE (東京:City {name: '東京', country: "日本", population: 14.0});
CREATE (`odd name`:`Display Label` {value: "quoted identifier", missing: NULL});
CREATE (snow:Symbol {name: "雪", code: 38634, enabled: TRUE});

MATCH (ada:Person)-->(friend:Person)
RETURN ada, friend;

MATCH (ada)<--(manager:Person)
RETURN manager.name;

MATCH (ada)-[knows:KNOWS]->(grace)
RETURN type(knows), startnode(knows), endnode(knows);

MATCH (ada)<-[incoming:MENTORS]-(grace)
RETURN incoming;

MATCH (ada)-[route:KNOWS|LIKES*1..4]->(other)
WHERE other.active = TRUE
RETURN route, other;

MATCH (a)-[maybe:RELATED?*2..5]-(b)
RETURN a, maybe, b;

MATCH (a)-[open:PATH*]->(b)
RETURN nodes(open), relationships(open), length(open);

MATCH (a)-[short:PATH?]->(b)
RETURN head(nodes(short)), last(nodes(short)), tail(nodes(short));

MATCH (person:Person)
WHERE person.age > 18 AND person.age <= 65
  AND person.name <> 'Nobody'
  AND person.name =~ '(?i)a.*'
  AND person.role IN ['admin', 'editor', 'reader']
  AND person.deleted IS NULL
RETURN person.name, person.age;

MATCH (person:Person)
WHERE person.active = TRUE OR person.pending = TRUE XOR person.blocked = FALSE
RETURN person;

MATCH (person:Person)
WHERE NOT(person.hidden) AND person.score / 2 + 7 * 3 - 1 >= 10
RETURN person.score % 5 AS remainder;

OPTIONAL MATCH (person)-[:OWNS]->(asset)
WITH person, asset
RETURN person.name, coalesce(asset.name, "none") AS assetName;

UNWIND range(1, 8) AS number
WITH number, CASE
  WHEN number = 1 THEN 'one'
  WHEN number < 4 THEN 'few'
  ELSE 'many'
END AS bucket
RETURN number, bucket
ORDER BY number DESC
SKIP 1
LIMIT 5;

MATCH (person:Person)
RETURN DISTINCT person.city AS city, count(*) AS residents
ORDER BY residents DESC;

MATCH (person:Person)
RETURN sum(person.score), avg(person.score), max(person.score), min(person.score),
       stdevp(person.score), percentileDisc(person.score),
       percentileCont(person.score), collect(person.name), count(person);

RETURN abs(-3), acos(0.5), asin(0.5), atan2(1, 2), cos(0), cot(1),
       degrees(pi()), exp(1), floor(3.9), haversin(0.5), log(10), log10(100),
       radians(180), rand(), round(2.6), sign(-9), sin(0), sqrt(81), tan(0);

RETURN str(42), replace('café', 'é', 'e'), substring('abcdef', 1, 3),
       left('東京', 1), right('rocket🚀', 1), ltrim(' x'), rtrim('x '),
       trim(' x '), lower('LOUD'), upper('quiet'), split('a,b,c', ',');

RETURN all(x IN range(1, 4) WHERE x > 0),
       any(x IN range(1, 4) WHERE x = 3),
       none(x IN range(1, 4) WHERE x < 0),
       single(x IN range(1, 4) WHERE x = 2);

RETURN extract(x IN range(1, 4) | x * x),
       filter(x IN range(1, 8) WHERE x % 2 = 0),
       reduce(total = 0, x IN range(1, 5) | total + x),
       labels(ada), id(ada), timestamp(), toint('42'), tofloat('3.5');

MERGE (service:Service {name: "syntax"})
ON CREATE SET service.created = timestamp(), service.state = 'new'
ON MATCH SET service.visits = coalesce(service.visits, 0) + 1
SET service:Observed
REMOVE service.legacy
RETURN service;

MATCH (obsolete:Temporary)
DELETE obsolete;

MATCH (detached:Temporary)
WITH detached
DELETE detached;

FOREACH (value IN range(1, 3) |
  CREATE (:Generated {value: value, label: 'item'})
);

LOAD CSV FROM 'file:///plain.csv' AS row
FIELDTERMINATOR ','
RETURN row;

LOAD CSV WITH HEADERS FROM "file:///unicode.csv" AS row
RETURN row.name, row.`postal code`;

USING PERIODIC COMMIT
LOAD CSV FROM 'file:///large.csv' AS line
CREATE (:Imported {name: line[0], value: line[1]});

CREATE INDEX ON :Person(name);
DROP INDEX ON :Person(name);
CREATE CONSTRAINT ON (person:Person) ASSERT person.id IS UNIQUE;
DROP CONSTRAINT ON (person:Person) ASSERT person.id IS UNIQUE;

MATCH (person:Person)
USING INDEX person:Person(name)
WHERE person.name = 'Ada'
RETURN person;

CALL db.labels()
YIELD label
RETURN label;

START old=node:people(name = "Ada")
RETURN old;

START old=node(42)
RETURN old;

START edge=relationship(7)
RETURN edge;

START edge=rel:links(kind = 'KNOWS')
RETURN edge;

MATCH (source)-[
  relation:TRANSFER|COPY*1..3
  {status: "ok", retries: 2, archived: FALSE}
]->(target)
RETURN source, relation, target;

CREATE (escaped:Text {
  single: 'quote\' and slash\\',
  double: "quote\" and tab\t",
  bmp: "café λ 東京 雪",
  astral: "🚀 𝌆"
});

MATCH (`odd name`)-[`relationship name`:`TYPE WITH SPACE`]->(`target node`)
RETURN `odd name`, `relationship name`, `target node`;

MATCH (a)--(b)
UNION
MATCH (a)-->(b)
RETURN a, b;

MATCH (node:Metric {bucket: 1}) RETURN node.value + 1 AS value;
MATCH (node:Metric {bucket: 2}) RETURN node.value + 2 AS value;
MATCH (node:Metric {bucket: 3}) RETURN node.value + 3 AS value;
MATCH (node:Metric {bucket: 4}) RETURN node.value + 4 AS value;
MATCH (node:Metric {bucket: 5}) RETURN node.value + 5 AS value;
MATCH (node:Metric {bucket: 6}) RETURN node.value + 6 AS value;
MATCH (node:Metric {bucket: 7}) RETURN node.value + 7 AS value;
MATCH (node:Metric {bucket: 8}) RETURN node.value + 8 AS value;
MATCH (node:Metric {bucket: 9}) RETURN node.value + 9 AS value;
MATCH (node:Metric {bucket: 10}) RETURN node.value + 10 AS value;
