package stress

import (
	"fmt"
	"regexp"
)

/*
Go stress fixture.

	Multi-line comment with non-ASCII text: café λ🚀.
*/
const raw = `line one
line two with "quotes" and λ🚀
${not_interpolation}`

const (
	stateUnknown = iota
	stateQueued
	stateRunning
	stateDone

	maxRetries       = 3
	permissionMask   = 0o755
	hexPattern       = 0xCA_FE
	measurement      = 6.022e23
	complexImpedance = 2.5 + 4i
	rocket           = '🚀'
	welcome          = "γειά, 世界, नमस्ते"
)

var (
	errStopped = fmt.Errorf("stopped at %s", "終点")
	primes     = []int{2, 3, 5, 7, 11}
	aliases    = map[string]string{"lambda": "λ", "music": "♫", "fox": "🦊"}
	zeroPair   Pair[int, string]
)

type (
	Identifier interface {
		~int64 | ~uint64 | ~string
	}
	Number interface {
		~int | ~int32 | ~int64 | ~float32 | ~float64
	}
	Pair[A, B any] struct {
		First  A `json:"first"`
		Second B `json:"second"`
	}
	Event struct {
		ID      uint64
		Name    string
		Payload map[string]any
		Tags    []string
	}
	Label   string
	Counter struct {
		value int
	}
	Envelope[T any] struct {
		Event
		Value T
		Meta  map[string]string
	}
	IntStringPair = Pair[int, string]
	PairEnvelope  = Envelope[IntStringPair]
)

func (label Label) String() string {
	return string(label)
}

func (counter *Counter) Add(delta int) int {
	counter.value += delta
	return counter.value
}

func (counter Counter) Value() int {
	return counter.value
}

func NewPair(first int, second string) IntStringPair {
	return IntStringPair{First: first, Second: second}
}

func Sum(values ...int) int {
	var total int
	for _, value := range values {
		total += value
	}
	return total
}

func Transform(values []string, convert func(string) int) []int {
	result := make([]int, 0, len(values))
	for _, value := range values {
		result = append(result, convert(value))
	}
	return result
}

func Ratio(total, count float64) float64 {
	return total / count
}

func literalExamples() (map[string]any, []rune) {
	values := map[string]any{
		"binary":  0b1010_0110,
		"octal":   0o640,
		"hex":     0x1.fp+8,
		"escaped": "tab:\t quote:\" snowman:\u2603 globe:\U0001F30D",
		"raw":     `C:\tmp\資料\nnot-a-newline`,
	}
	runes := []rune{'a', '\n', '\u03BB', '界', '🧪'}
	return values, runes
}

func compositeExamples() []PairEnvelope {
	first := IntStringPair{First: 1, Second: "uno"}
	second := NewPair(2, "dos")
	return []PairEnvelope{
		{Event: Event{ID: 1, Name: "α", Tags: []string{"new", "東京"}}, Value: first},
		{Event: Event{ID: 2, Name: "β", Payload: map[string]any{"ok": true}}, Value: second},
	}
}

func launch(done <-chan struct{}, values []string) <-chan Event {
	out := make(chan Event, len(values))
	go func() {
		defer close(out)
		for index, value := range values {
			select {
			case <-done:
				return
			case out <- Event{ID: uint64(index + 1), Name: value}:
			}
		}
	}()
	return out
}

func guardedRatio(total, count float64) (result float64, err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = fmt.Errorf("ratio panic: %v", recovered)
		}
	}()
	if count == 0 {
		panic("division by zero: ∞")
	}
	return Ratio(total, count), nil
}

func describe(value any) string {
	if stringer, ok := value.(fmt.Stringer); ok {
		return "stringer:" + stringer.String()
	}
	switch typed := value.(type) {
	case nil:
		return "<nil>"
	case string:
		return "text:" + typed
	case int, int32, int64:
		return fmt.Sprintf("integer:%v", typed)
	case []byte:
		return fmt.Sprintf("bytes:%x", typed)
	default:
		return fmt.Sprintf("%T", typed)
	}
}

func search(grid [][]int, wanted int) (int, int, bool) {
rows:
	for row := 0; row < len(grid); row++ {
		for column, value := range grid[row] {
			switch {
			case value == wanted:
				return row, column, true
			case value < 0:
				continue rows
			}
		}
	}
	return -1, -1, false
}

func closureExamples(seed int) (func(int) int, func() int) {
	total := seed
	add := func(delta int) int {
		total += delta
		return total
	}
	return add, func() int { return total }
}

func main() {
	re := regexp.MustCompile(`^/api/[\p{L}-]+$`)
	fmt.Println(re.MatchString("/api/café"), Ratio(42, 2), raw)
}
