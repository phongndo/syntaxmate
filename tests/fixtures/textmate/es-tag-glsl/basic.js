const tint = "vec3(0.3, 0.7, 1.0)";
const exposure = 1.25;

export const fragmentSource = glsl`
  #version 300 es
  precision highp float;
  // café 東京 🚀 𝌆
  uniform sampler2D u_texture;
  in vec2 v_uv;
  out vec4 fragColor;

  void main() {
    vec3 color = texture(u_texture, v_uv).rgb * ${tint};
    fragColor = vec4(color * ${exposure}, 1.0);
  }
`;

const size = 16;
export const computeSource = /* inline-glsl */ `
  layout(local_size_x = ${size}) in;
  /* Multiline shader comment with an escaped marker: \u2603. */
  void main() { uint index = gl_GlobalInvocationID.x; }
`;

console.log(fragmentSource.length + computeSource.length);
