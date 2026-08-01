(module $greetings
  ;; Small UTF-8 greeting: café 東京 λ 🚀 𝌆
  (type $binary (func (param i32 i32) (result i32)))
  (import "host" "log" (func $log (param i32 i32)))
  (memory (export "memory") 1)
  (global $calls (mut i32) (i32.const 0))
  (; The bytes spell "Hello"; this block comment is fully closed. ;)
  (data (i32.const 0) "Hello, \e4\b8\96\e7\95\8c 🚀 𝌆\00")
  (func $add (export "add") (type $binary) (param $left i32) (param $right i32) (result i32)
    global.get $calls
    i32.const 1
    i32.add
    global.set $calls
    local.get $left
    local.get $right
    i32.add)
  (func $main (export "main")
    i32.const 0
    i32.const 24
    call $log)
  (start $main))
