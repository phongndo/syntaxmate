package fixture

import (
	"fmt"
	"strings"
)

type Greeting struct {
	Name  string
	Count int
}

func (g Greeting) Message() string {
	return strings.Repeat("Hello "+g.Name+" 🚀 ", g.Count)
}

func main() {
	items := []string{"Go", "TextMate"}
	for index, item := range items {
		fmt.Printf("%d: %s\n", index, item)
	}
	fmt.Println(Greeting{Name: "world", Count: 2}.Message())
}
