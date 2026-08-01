(module $runtime
  ;; Broad WebAssembly grammar fixture: café, Ελληνικά, 東京, 🚀 and 𝌆.
  (; Nested-looking prose stays in one closed block comment.
     It exercises a multiline state with BMP λ and astral 🚀 / 𝌆. ;)

  (type $unary-i32 (func (param i32) (result i32)))
  (type $binary-i64 (func (param i64 i64) (result i64)))
  (type $callback (func (param externref) (result i32)))
  (type $pair (struct (field i32) (field (mut i64))))
  (type $bytes (array (mut i8)))
  (import "env" "print" (func $print (param i32 i32)))
  (import "env" "seed" (global $seed i64))
  (import "env" "callbacks" (table $callbacks 4 funcref))
  (import "env" "shared" (memory $shared 1 8 shared))
  (memory $scratch 2 16)
  (table $dispatch 8 32 funcref)
  (global $counter (mut i32) (i32.const 0))
  (global $ratio f64 (f64.const 0x1.921fb54442d18p+1))
  (global $nothing externref (ref.null extern))
  (export "scratch" (memory $scratch))
  (export "dispatch" (table $dispatch))

  ;; Strings include escapes, BMP text, and surrogate-pair-sized characters.
  (data $hello (i32.const 0) "café λ 東京 — 🚀 𝌆\0a\00")
  (data $escaped (i32.const 64) "quote=\22 slash=\5c tab=\09\00")
  (data passive "deferred-data")
  (elem $initial (i32.const 0) func $sum32 $countdown $classify)

  (func $sum32 (export "sum32") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)

  (func $countdown (param $n i32) (result i32)
    (local $acc i32)
    i32.const 0
    local.set $acc
    block $done
      loop $again
        local.get $n
        i32.eqz
        br_if $done
        local.get $acc
        local.get $n
        i32.add
        local.set $acc
        local.get $n
        i32.const 1
        i32.sub
        local.tee $n
        br_if $again
      end
    end
    local.get $acc)

  (func $classify (param $value i32) (result i32)
    local.get $value
    if (result i32)
      then i32.const 1
      else i32.const -1
    end)

  (func $integer-math (param $x i64) (result i64)
    local.get $x
    i64.const 0xffff
    i64.and
    i64.const 3
    i64.rotl
    i64.const 7
    i64.div_u
    i64.extend32_s)

  (func $float-math (param $x f64) (result f64)
    local.get $x
    f64.abs
    f64.const -inf
    f64.max
    f64.const nan:0x400000
    f64.copysign
    f64.sqrt)

  (func $conversions (param $x f32) (result i32)
    local.get $x
    i32.trunc_sat_f32_s
    i32.extend16_s)

  (func $memory-ops (param $address i32) (param $value i64)
    local.get $address
    local.get $value
    i64.store32 offset=4 align=4
    local.get $address
    i64.load32_u offset=4 align=4
    drop
    i32.const 96
    i32.const 0
    i32.const 16
    memory.copy
    i32.const 112
    i32.const 32
    i32.const 8
    memory.fill
    memory.size
    drop)

  (func $bulk-init
    i32.const 128
    i32.const 0
    i32.const 13
    memory.init 2
    memory.drop 2
    i32.const 4
    i32.const 0
    i32.const 3
    table.init $initial
    table.size $dispatch
    drop)

  (func $vectors (result v128)
    (local $mask v128)
    v128.const i8x16 0x00 0x01 0x02 0x03 0x04 0x05 0x06 0x07 0x08 0x09 0x0a 0x0b 0x0c 0x0d 0x0e 0x0f
    local.set $mask
    v128.const i32x4 0x00000001 0x00000002 0x00000003 0x00000004
    local.get $mask
    i8x16.shuffle 0 1 2 3 20 21 22 23 8 9 10 11 28 29 30 31
    i8x16.add
    i16x8.widen_low_i8x16_s
    i16x8.shr_u
    i32x4.trunc_sat_f32x4_s
    f32x4.convert_i32x4_u
    v128.bitselect)

  (func $atomic-ops (param $p i32) (result i32)
    local.get $p
    i32.atomic.load align=4
    local.get $p
    i32.const 1
    i32.atomic.rmw.add align=4
    i32.add
    local.get $p
    i64.const 0
    i64.const 1000
    i64.atomic.wait
    drop
    local.get $p
    i32.const 1
    atomic.notify
    drop
    atomic.fence)

  (func $references (param $item externref) (result i32)
    local.get $item
    ref.is_null
    if (result i32)
      then i32.const 0
      else local.get $item
           call_indirect (type $callback)
    end)

  (func $table-ops (param $slot i32)
    local.get $slot
    table.get $dispatch
    drop
    i32.const 0
    ref.null func
    i32.const 2
    table.fill $dispatch
    i32.const 1
    table.grow $dispatch
    drop)

  (func $tail (param $x i32) (result i32)
    local.get $x
    return_call $countdown)

  (func $exceptions (param $code i32) (result i32)
    try (result i32)
      local.get $code
      throw $fault
    catch $fault
      i32.const -2
    end)
  (event $fault (param i32))

  (func $gc-demo (param $object eqref) (result i32)
    local.get $object
    ref.test $pair
    drop
    i32.const 7
    i64.const 9
    struct.new_canon $pair
    struct.get $pair 0)

  (func $branch-table (param $choice i32) (result i32)
    block $fallback
      block $one
        block $zero
          local.get $choice
          br_table $zero $one $fallback
        end
        i32.const 10
        return
      end
      i32.const 20
      return
    end
    i32.const 30)

  (func $main
    i32.const 0
    i32.const 25
    call $print
    i32.const 5
    call $countdown
    global.set $counter
    nop)
  (export "run" (func $main))
  (start $main))
