package fixture

import "strings"

// A compact, concrete configuration with Unicode in identifiers and values.
#Greeting: {
	message:  string
	excited?: bool
}

café: "coffee"
日本語: "こんにちは"
name: "Ada 🚀 𝌆"
greeting: #Greeting & {
	message: "Hello, \(name)!"
	excited: true
}
upper: strings.ToUpper(greeting.message)
values: [1, 2, 3, ...int]
selected: [for value in values if value > 1 {value * 2}]
status: *"ready" | "waiting"
