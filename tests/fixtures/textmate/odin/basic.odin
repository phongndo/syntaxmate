package main

import "core:fmt"

Mode :: distinct int
IDLE :: Mode(0)
ACTIVE :: Mode(1)

scale :: proc(value: f64, factor: f64 = 2.0) -> f64 {
    return value * factor
}

main :: proc() {
    // Composite literals carry café and astral symbols 🚀 𝌆.
    labels := [?]string{"café 🚀", "orbit 𝌆"}
    values := [?]f64{0x2a, 3.5e1}
    total := 0.0
    for label, index in labels {
        total += scale(values[index])
        fmt.printf("%d %s: %.1f\n", index, label, values[index])
    }
    mode := ACTIVE
    fmt.println(mode, total)
}
