# Hurl stress fixture: varied requests and deliberately closed multiline bodies.
# Unicode sentinels: café, λ, 東京, 🚀, 𝌆.

# Query parameters, cookies, options, captures, and assertions
GET https://api.example.test/v2/search?q={{term}}
Accept: application/json
Accept-Language: fr, ja;q=0.8
X-Debug-Label: "café\\t東京"
[QueryStringParams]
page: 2
limit: 25
symbol: λ
[Cookies]
session: {{session_id}}
theme: dark
[Options]
retry: 3
retry-interval: 100
HTTP/2 200
Content-Type: application/json; charset=utf-8
X-Trace-Id: trace-search-001
[Captures]
search_trace: header "X-Trace-Id"
first_id: jsonpath "$[0].id"
[Asserts]
status == 200
header "Content-Type" contains "application/json"
jsonpath "$[0].active" == true

# JSON request body with strings, placeholders, and escapes
POST https://api.example.test/v2/projects
Authorization: Bearer {{token}}
Content-Type: application/json
Idempotency-Key: create-{{request_id}}
X-Escapes: "quote=\" hash=\# slash=\\ newline=\n"
[Options]
timeout: 2500
```json
{
  "name": "Café launch 🚀",
  "city": "東京",
  "owner": "{{owner}}",
  "notes": "lambda λ and tetragram 𝌆",
  "escaped": "line one\nline two\tindented",
  "enabled": true
}
```
HTTP/1.1 201
Location: https://api.example.test/v2/projects/prj-42
Content-Type: application/json
[Captures]
project_id: jsonpath "$.id"
[Asserts]
status == 201
jsonpath "$.name" == "Café launch 🚀"

# XML body and conditional response checks
PUT https://api.example.test/v2/catalog/items/sku-7
Content-Type: application/xml; charset=utf-8
If-Match: "rev-{{revision}}"
```xml
<?xml version="1.0" encoding="UTF-8"?>
<item sku="sku-7">
  <name>Café crème</name>
  <warehouse city="東京">A-12</warehouse>
  <marker>λ 🚀 𝌆</marker>
</item>
```
HTTP/1.1 204
ETag: "rev-8"
[Captures]
catalog_etag: header "ETag"
[Asserts]
status == 204
header "ETag" matches "rev-[0-9]+"

# GraphQL body remains balanced across several lines
PATCH https://api.example.test/graphql
Authorization: Bearer {{token}}
Content-Type: application/graphql
```graphql
mutation RenameProject($id: ID!, $name: String!) {
  renameProject(input: {id: $id, name: $name}) {
    project {
      id
      name
    }
    errors { field message }
  }
}
```
HTTP 200
Content-Type: application/json
[Asserts]
status == 200
jsonpath "$.data.renameProject.errors" count == 0

# CSV-shaped text body and the short Query section alias
POST https://imports.example.test/v1/rows
Content-Type: text/csv; charset=utf-8
[Query]
mode: append
source: fixture
```text
id,name,city,marker
1,"Café rouge",Paris,λ
2,"Launch 🚀",東京,𝌆
```
HTTP/1.0 202
Content-Type: application/json
[Captures]
job_url: header "Location"
[Asserts]
status == 202

# Hex fenced body followed by a fully terminated inline hex body
POST https://blobs.example.test/v1/fenced-hex
Content-Type: application/octet-stream
```hex
48656c6c6f2c20
4865726c21
```
HTTP/1.1 201
[Asserts]
status == 201

POST https://blobs.example.test/v1/inline-hex
Content-Type: application/octet-stream
hex,48656c6c6f
2d7b7b626c6f625f69647d7d;
HTTP/1.1 201
X-Blob-Id: blob-hex-02
[Captures]
hex_blob: header "X-Blob-Id"

# Base64 fenced body and a terminated multiline base64 body
POST https://blobs.example.test/v1/fenced-base64
Content-Type: application/octet-stream
```base64
SGVsbG8sIEh1cmwh
Y2Fmw6ktzrvmnbHkuqwt8J+agC3wnYyG
```
HTTP/1.1 201
[Asserts]
status == 201

POST https://blobs.example.test/v1/inline-base64
Content-Type: application/octet-stream
base64,U3RyZXNz
LXt7YmxvYl9pZH19;
HTTP/1.1 201
X-Blob-Id: blob-b64-02
[Captures]
base64_blob: header "X-Blob-Id"

# Generic fenced text body with modifier captures
POST https://notes.example.test/v1/notes
Content-Type: text/plain; charset=utf-8
```text,escape
First line for {{reader}}.
Second line says café, λ, 東京, 🚀, and 𝌆.
The fence is closed on the next line.
```
HTTP/1.1 200
[Asserts]
status == 200

# Backtick body, BasicAuth, and escaped delimiters
POST https://auth.example.test/v1/session
Content-Type: text/plain
[BasicAuth]
{{username}}: {{password}}
[Options]
compressed: true
`login={{username}}; note=quote:\" slash:\\ tick:\` newline:\n`
HTTP/1.1 200
Set-Cookie: session=fixture; Secure; HttpOnly
[Captures]
login_cookie: cookie "session"
[Asserts]
status == 200

# Form sections, including grammar-supported short aliases
POST https://forms.example.test/v1/profile
Content-Type: application/x-www-form-urlencoded
[FormParams]
display_name: "Café User"
home_city: 東京
[Form]
motto: "λ to the stars 🚀"
HTTP/1.1 200
[Asserts]
status == 200

POST https://uploads.example.test/v1/artifacts
[MultipartFormData]
title: "Résumé 𝌆"
file: file,fixtures/report.txt;
[Multipart]
description: "Hand-reviewable upload"
HTTP/1.1 201
Location: https://uploads.example.test/v1/artifacts/a-1

# Remaining request methods and protocol/status forms
HEAD https://api.example.test/v2/projects/prj-42
If-None-Match: "rev-8"
HTTP/1.1 304
ETag: "rev-8"

OPTIONS https://api.example.test/v2/projects
Origin: https://console.example.test
Access-Control-Request-Method: DELETE
HTTP/1.1 204
Allow: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, TRACE

TRACE https://api.example.test/diagnostics
Max-Forwards: 1
HTTP/1.1 200
Content-Type: message/http

CONNECT https://tunnel.example.test/session
Proxy-Authorization: Basic {{proxy_token}}
HTTP/1.1 200

DELETE https://api.example.test/v2/projects/prj-old
Authorization: Bearer {{token}}
[Options]
location: true
HTTP/1.1 202
[Asserts]
status == 202

# Final plain response verifies that no multiline rule leaks to EOF.
GET http://status.example.test/final
X-Final-Placeholder: {{final_value}}
HTTP 200
X-Unicode-Echo: café-λ-東京-🚀-𝌆
[Asserts]
status == 200
# End of closed Hurl stress fixture.
