// MLIR: café, λ, and an astral rocket 🚀
#identity = affine_map<(d0) -> (d0)>
!vector4 = vector<4xf32>
module attributes {demo.title = "orbit 🚀\n", demo.enabled = true} {
  func.func @mix(%input: tensor<4xf32>, %flag: i1) -> tensor<4xf32> {
    %c0 = arith.constant 0 : index
    %bias = arith.constant 1.25e+1 : f32
    %value = tensor.extract %input[%c0] : tensor<4xf32>
    %sum = arith.addf %value, %bias : f32
    %chosen = scf.if %flag -> (f32) {
      scf.yield %sum : f32
    } else {
      %zero = arith.constant 0.0 : f32
      scf.yield %zero : f32
    }
    %out = tensor.insert %chosen into %input[%c0] : tensor<4xf32>
    "demo.observe"(%chosen) {kind = "result", marker = unit} : (f32) -> ()
    cf.br ^done(%out : tensor<4xf32>)
  ^done(%final: tensor<4xf32>):
    return %final : tensor<4xf32>
  }
}
