/** Basic vertex shader: café, 日本語, 🚀, and 𝌆. */
struct VertexInput {
  @location(0) position: vec3f,
  @location(1) color: vec4f,
}

struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
}

const SCALE: f32 = 1.25;

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
  var output: VertexOutput;
  output.position = vec4f(input.position * SCALE, 1.0);
  output.color = input.color;
  return output;
}

// Closed line comment with astral symbols 🚀 𝌆.
