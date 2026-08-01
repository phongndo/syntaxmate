package coverage

import (
	"list"
	"math"
	文字列 "strings"
)

import codec "encoding/json:json"

// Definitions, field markers, constraints, and attributes are ordinary CUE.
@experiment(explicitopen)
#Metadata: {
	name!:     string
	owner?:    string
	labels:    [string]: string
	createdAt: string
}

#Port: int & >=1 & <=65535

#Endpoint: {
	host: string
	port: #Port | *8080
	tls:  bool | *true
}

#Service: {
	metadata: #Metadata
	endpoint: #Endpoint
	replicas: uint8 & <=100 | *3
	tags:     [...string]
	...  // Consumers may add deployment-specific fields.
}

/* A block comment can contain punctuation such as {}, [], (), and "quotes".
   It can also contain BMP 日本語 and astral symbols 🚀 𝌆 without nesting. */
name:     "café-api"
日本語:    "設定"
rocket:   "🚀"
tetagram: "𝌆"
_cache:   "hidden field"

service: #Service & {
	metadata: {
		name:      name
		owner:     "platform"
		labels:    {team: "syntax", locale: "日本語"}
		createdAt: "2026-07-12T10:30:00Z"
	}
	endpoint: {host: "api.example.test", port: 8443}
	tags: ["cue", "textmate", "café"]
} @service(owner=platform, policy(mode="strict",tier=core))

// Every current, valid quoted form is represented below.
plain: "line one\nline two\t\"quoted\" \\ slash"
escapedUnicode: "BMP=\u65E5, astral=\U0001F680"
interpolated: "service=\(service.metadata.name), port=\(service.endpoint.port)"
computedText: "upper=\(文字列.ToUpper(name)); tags=\(len(service.tags))"

multiline: """
	CUE configuration for café and 日本語.
	Launch symbol: \(rocket); musical symbol: 𝌆.
	A literal tab follows:\tfinished.
	"""

bytes: 'CUE\n\x21\041'
bytesUnicode: 'café 日本語 🚀 𝌆'
bytesInterpolated: 'name=\(name)'

multilineBytes: '''
	bytes line one
	bytes line two: \(name)
	'''

rawText: #"C:\work\cue says "hello"; service=\#(name)"#
rawBytes: #'C:\binary\path; rocket=\#(rocket)'#

rawMultiline: #"""
	Backslashes stay literal: C:\temp\new.
	Ordinary \(text) is not interpolation here.
	Hash interpolation is active: \#(service.metadata.owner).
	"""#

rawMultilineBytes: #'''
	Raw byte text: \x43 and "quotes".
	Interpolated Unicode: \#(日本語) 🚀 𝌆.
	'''#

// Numeric literals cover bases, separators, exponents, fractions, and suffixes.
decimal:       1_000_000
binary:        0b1010_0110
octal:         0o755
hexadecimal:   0xCAFE_BABE
fraction:      12.375
leadingDot:    .625
scientific:    6.022e+23
smallExponent: 1e-9
decimalScale:  12K
binaryScale:   4Mi
scaledFloat:   1.5G

// Scalar types and language constants.
anything:  _
never:     _|_
empty:     null
enabled:   true
disabled:  false
flag:      bool
count:     uint64
signed:    int32
ratio:     float64
quantity:  number
text:      string
blob:      bytes
runeValue: rune & 0x1F680

// Arithmetic, word operators, comparisons, logic, and set operations.
sum:       20 + 4 - 3
product:   6 * 7 / 2
floorDiv:  17 div 5
modulus:   17 mod 5
quotient:  17 quo 5
remainder: -17 rem 5
equal:     count == 1_000_000
unequal:   name != "other"
ordered:   decimal >= 10 && decimal <= 2_000_000
strict:    fraction > 10 && leadingDot < 1
matched:   name =~ "^[[:alpha:]]" && name !~ "\\s"
logic:     enabled && !disabled || service.endpoint.tls
unified:   ({a: int} & {a: >=1})
alternate: ({kind: "one"} | {kind: "two"})

// Defaults, optional fields, required fields, and aliases.
mode: *"production" | "staging" | "debug"
settings: cfg={
	mode:     mode
	retries?: int & >=0 & <=10
	token!:   string
}
settingsCopy: cfg

indexed: [first=10, second=20, first + second]
openList: ["fixed", 2, ...string | int]
openStruct: {
	known: string
	...
}

// Pattern constraints and dynamic labels use bracket and interpolation syntax.
environment: {
	[=~"^(dev|stage|prod)$"]: #Endpoint
	prod: {host: "prod.example.test", port: 443}
}

localized: {
	for language, greeting in {
		en: "hello"
		fr: "bonjour"
		日本語: "こんにちは"
	} {
		"\(language)-message": greeting
	}
}

rawPorts: [
	{name: "http", port: 80, enabled: true},
	{name: "https", port: 443, enabled: true},
	{name: "admin", port: 9000, enabled: false},
]

activePorts: [
	for index, item in rawPorts
	if item.enabled {
		let doubled = item.port * 2
		{
			index:  index
			name:   item.name
			port:   item.port
			double: doubled
		}
	},
]

// Built-ins, selectors, module calls, parentheses, and comma punctuation.
tagCount: len(service.tags)
closedEndpoint: close({
	host: "localhost"
	port: 3000
})
combined: and([{value: int}, {value: >=0}, {value: <=10}])
selectedType: or([string, bytes])
sorted: list.Sort([3, 1, 2], list.Ascending)
rounded: math.Floor(3.75)
decoded: codec.Unmarshal('{"ok":true,"message":"日本語 🚀"}')
nestedMember: service.metadata.labels.team
parenthesized: (sum + product) * (floorDiv + modulus)

// A final attribute includes unquoted, bound, quoted, and nested elements.
result: {
	ready: logic
	ports: activePorts
	message: computedText
} @report(format="text", owner=syntax, flags(unicode,interpolation))
