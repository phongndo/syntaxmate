// Compact GDShader coverage: 東京, λ, café, 🚀, and 𝌆.
shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back;
/* A block comment crosses
   a physical line boundary. */
struct SurfaceData {
    highp vec3 normal;
    float weights[4];
};
uniform sampler2D albedo_texture : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
global uniform vec4 tint : hint_color;
instance uniform float roughness : hint_range(0.0, 1.0, 0.05);
varying flat vec3 world_normal;
const float EPSILON = 1.0E-4F;
uvec2 flags = uvec2(0x2A, 7u);
float helper(in vec3 p, out bool ok) {
    ok = p.x >= 0.0 && true;
    return ok ? p.y : -PI;
}
void fragment() {
    vec4 sampled = texture(albedo_texture, UV);
    ALBEDO = sampled.rgb * tint.rgb;
    ROUGHNESS = clamp(roughness + TAU / 100.0, 0.0, 1.0);
    if (sampled.a <= 0.01) { discard; }
    else { ALPHA = sampled.a; }
}
