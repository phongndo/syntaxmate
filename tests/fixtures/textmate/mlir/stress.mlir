// End-to-end lowering fixture: naïve façade, λ-shaped tiles, and 東京 data.
// Astral characters stay in safe text: compiler pipeline 🧭, result rocket 🚀.
#tile = affine_map<(d0, d1)[s0] -> (d0 floordiv 8, d1 mod 8, s0 ceildiv 4)>
#linear = affine_map<(d0, d1)[s0] -> (d0 * s0 + d1)>
#interior = affine_set<(d0, d1)[m, n] : (d0 >= 0, d1 >= 0, -d0 + m - 1 >= 0, -d1 + n - 1 >= 0)>
#row_major = #demo.blocked<
  {order = [1, 0], warps = 4, lanes = 32, exact = true}
>
#seed = dense<[
  [1.0, -2.5, 3.25, 0.0],
  [4.5, 5.0, -6.75, 8.125]
]> : tensor<2x4xf32>
#coordinates = sparse<[[0, 1], [1, 3]], [2.0, -1.0]> : tensor<2x4xf32>
#payload = #demo.payload<opaque<"demo", "0xCAFE">, type = tensor<2xi8>>
#pipeline = #demo.pipeline<stages = 3, policy = "latency", fallback = @scalar_fallback>
!matrix = tensor<?x?xf32>
!tile_buffer = memref<8x8xf32, #tile, 3>
!scalable = vector<[4]x8xf32>
!packet = !demo.packet<
  tuple<i64, f32, complex<f32>>,
  #demo.encoding<kind = "compact", version = 2>
>
!callback = !llvm.ptr

module @image_pipeline attributes {
  demo.title = "Café 東京 — launch 🚀",
  demo.description = "normalize\nthen tile\tand emit \"ready\"",
  demo.enabled = true,
  demo.dry_run = false,
  demo.marker = unit,
  demo.revision = 0x2A,
  demo.scale = 6.25e-2,
  demo.entry = @run,
  demo.fallbacks = [@scalar_fallback, @debug::@trace],
  demo.options = {
    layout = #row_major,
    modes = ["fast", "precise"],
    thresholds = [0.125, 1.0, 9.5e+1],
    nested = {owner = "Δ-team", visible = true}
  },
  demo.pipeline = #pipeline
} {
  memref.global "private" @kernel_name : memref<12xi8> = dense<0> {alignment = 16 : i64}
  memref.global @bias : memref<4xf32> = dense<[0.25, -0.5, 1.0, 2.0]>

  func.func private @sink(!packet) attributes {llvm.emit_c_interface}
  func.func private @scalar_fallback(%x: f32) -> f32

  func.func @scalar_math(%lhs: i32, %rhs: i32, %gate: i1) -> (i32, f64) {
    %zero = arith.constant 0 : index
    %negative = arith.constant -17 : i32
    %signed = "demo.signed_literal"() {value = -17 : si32} : () -> si32
    %unsigned = "demo.unsigned_literal"() {value = 255 : ui16} : () -> ui16
    %bits = arith.constant 0xDEADBEEF : i64
    %tiny = arith.constant 1.5e-4 : f16
    %brain = arith.constant 2.0 : bf16
    %wide = arith.constant -3.1415926535 : f64
    %extended = arith.constant 0.0 : f80
    %quad = arith.constant 1.0 : f128
    %none = ub.poison : none
    %sum, %overflow = arith.addui_extended %lhs, %rhs : i32, i1
    %difference = arith.subi %sum, %negative : i32
    %selected = arith.select %gate, %difference, %lhs : i32
    %as_float = arith.sitofp %selected : i32 to f64
    %scaled = arith.mulf %as_float, %wide : f64
    %complex = complex.create %tiny, %tiny : complex<f16>
    "demo.scalar_snapshot"(%zero, %signed, %unsigned, %bits, %brain, %extended, %quad, %none, %overflow, %complex) {
      label = "scalar α snapshot", severity = #demo.level<info>
    } : (index, si32, ui16, i64, bf16, f80, f128, none, i1, complex<f16>) -> ()
    return %selected, %scaled : i32, f64
  }

  func.func @tiled_add(
      %left: memref<?x?xf32, strided<[?, 1], offset: ?>>,
      %right: memref<?x?xf32>,
      %output: memref<?x?xf32>,
      %rows: index,
      %cols: index) attributes {
        llvm.emit_c_interface,
        demo.schedule = #demo.schedule<map = #tile, vector_width = 8>
      } {
    affine.for %i = 0 to %rows step 4 {
      affine.for %j = 0 to %cols {
        affine.if #interior(%i, %j)[%rows, %cols] {
          %a = affine.load %left[%i, %j] : memref<?x?xf32, strided<[?, 1], offset: ?>>
          %b = affine.load %right[%i, %j] : memref<?x?xf32>
          %total = arith.addf %a, %b : f32
          affine.store %total, %output[%i, %j] : memref<?x?xf32>
        } else {
          // Boundary work is intentionally empty; Ω marks the slow path.
        }
      }
    }
    return
  }

  func.func @reduce_until(%initial: i32, %limit: i32) -> i32 {
    %one = arith.constant 1 : i32
    %four = arith.constant 4 : index
    %acc = scf.for %iv = %four to %four step %four
        iter_args(%running = %initial) -> (i32) {
      %next = arith.addi %running, %one : i32
      scf.yield %next : i32
    }
    %answer = scf.while (%current = %acc) : (i32) -> i32 {
      %keep_going = arith.cmpi slt, %current, %limit : i32
      scf.condition(%keep_going) %current : i32
    } do {
    ^advance(%value: i32):
      %incremented = arith.addi %value, %one : i32
      scf.yield %incremented : i32
    }
    %positive = arith.cmpi sgt, %answer, %initial : i32
    %checked = scf.if %positive -> (i32) {
      scf.yield %answer : i32
    } else {
      scf.yield %initial : i32
    }
    return %checked : i32
  }

  func.func @dispatch(%tag: i32, %value: f32) -> f32 {
    %is_zero = arith.cmpi eq, %tag, %tag : i32
    cf.cond_br %is_zero, ^fast(%value : f32), ^classify
  ^fast(%candidate: f32):
    %two = arith.constant 2.0 : f32
    %doubled = arith.mulf %candidate, %two : f32
    cf.br ^merge(%doubled : f32)
  ^classify:
    cf.switch %tag : i32, [
      default: ^merge(%value : f32),
      7: ^cold,
      0x10: ^cold
    ]
  ^cold:
    %negated = arith.negf %value : f32
    cf.br ^merge(%negated : f32)
  ^merge(%result: f32):
    return %result : f32
  }

  func.func @tensor_stage(%input: !matrix, %pad: f32) -> !matrix {
    %c0 = arith.constant 0 : index
    %c1 = arith.constant 1 : index
    %rows = tensor.dim %input, %c0 : tensor<?x?xf32>
    %cols = tensor.dim %input, %c1 : tensor<?x?xf32>
    %empty = tensor.empty(%rows, %cols) : tensor<?x?xf32>
    %filled = linalg.fill ins(%pad : f32) outs(%empty : tensor<?x?xf32>) -> tensor<?x?xf32>
    %slice = tensor.extract_slice %input[%c0, %c0]
        [%rows, %cols] [%c1, %c1]
        : tensor<?x?xf32> to tensor<?x?xf32>
    %combined = linalg.add
        ins(%slice, %filled : tensor<?x?xf32>, tensor<?x?xf32>)
        outs(%empty : tensor<?x?xf32>) -> tensor<?x?xf32>
    %first = tensor.extract %combined[%c0, %c0] : tensor<?x?xf32>
    %updated = tensor.insert %first into %combined[%c0, %c0] : tensor<?x?xf32>
    return %updated : tensor<?x?xf32>
  }

  func.func @vector_stage(%source: memref<?xf32>, %base: index) -> vector<8xf32> {
    %c0 = arith.constant 0 : index
    %mask = vector.create_mask %base : vector<8xi1>
    %padding = arith.constant 0.0 : f32
    %loaded = vector.transfer_read %source[%base], %padding, %mask
        {in_bounds = [true]} : memref<?xf32>, vector<8xf32>
    %steps = vector.step : vector<8xindex>
    %indices = arith.index_cast %steps : vector<8xindex> to vector<8xi32>
    %cast = arith.sitofp %indices : vector<8xi32> to vector<8xf32>
    %product = arith.mulf %loaded, %cast : vector<8xf32>
    %low, %high = vector.deinterleave %product : vector<8xf32> -> vector<4xf32>
    %joined = vector.interleave %high, %low : vector<4xf32> -> vector<8xf32>
    vector.transfer_write %joined, %source[%c0] {in_bounds = [false]}
        : vector<8xf32>, memref<?xf32>
    return %joined : vector<8xf32>
  }

  func.func @buffer_lifetime(%size: index, %alignment: i64) {
    %buffer = memref.alloc(%size) {alignment = 64 : i64} : memref<?xf32>
    %view = memref.subview %buffer[0] [%size] [1]
        : memref<?xf32> to memref<?xf32, strided<[1]>>
    %cast = memref.cast %view
        : memref<?xf32, strided<[1]>> to memref<?xf32>
    memref.assume_alignment %cast, 64 : memref<?xf32>
    "demo.consume_buffer"(%cast, %alignment) <{
      effects = [#demo.effect<read>, #demo.effect<write>],
      provenance = {file = "sensor_μ.raw", line = 42, trusted = false}
    }> : (memref<?xf32>, i64) -> ()
    memref.dealloc %buffer : memref<?xf32>
    return
  }

  func.func @transaction(%input: !packet) -> !packet {
    %result = "demo.transaction"(%input) ({
    ^prepare(%item: !packet):
      %valid = "demo.validate"(%item) {strict = true} : (!packet) -> i1
      "demo.note"(%valid) {message = "готово ✅"} : (i1) -> ()
      "demo.commit"(%item)[^finish] : (!packet) -> ()
    ^finish:
      "demo.region_yield"(%input) : (!packet) -> ()
    }) {
      isolation = #demo.isolation<snapshot>,
      retries = 3,
      tags = ["io", "atomic", "🧪"]
    } : (!packet) -> !packet
    func.call @sink(%result) : (!packet) -> ()
    return %result : !packet
  }

  func.func @run(%arg: tensor<?x?xf32>, %flag: i1) -> tensor<?x?xf32> {
    %zero = arith.constant 0.0 : f32
    %normalized = func.call @tensor_stage(%arg, %zero) : (tensor<?x?xf32>, f32) -> tensor<?x?xf32>
    "demo.trace"(%normalized) {
      event = "pipeline.done 🚀",
      source = @image_pipeline::@run,
      debug = {enabled = true, note = "café λ"}
    } : (tensor<?x?xf32>) -> ()
    return %normalized : tensor<?x?xf32>
  }
}
