# Basic Hurl request, response, captures, and assertions
POST https://api.example.test/v1/greetings/{{user_id}}
Accept: application/json
Content-Type: application/json; charset=utf-8
X-Trace: "basic\ntrace-{{trace_id}}"
[QueryStringParams]
locale: fr-FR
label: café-λ-東京-🚀-𝌆
[Options]
retry: 2
timeout: 1000
```json
{
  "message": "Bonjour {{name}}",
  "escaped": "quote: \"; slash: \\; tab: \t"
}
```
HTTP/1.1 201
Content-Type: application/json
X-Request-Id: req-basic-001
[Captures]
request_id: header "X-Request-Id"
[Asserts]
status == 201
header "Content-Type" startsWith "application/json"
jsonpath "$.message" == "Bonjour Ada"
# Every multiline construct above is closed.
