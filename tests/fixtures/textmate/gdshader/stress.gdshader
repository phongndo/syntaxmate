// Grammar-driven spatial shader stress fixture: 東京 λ 🚀 𝌆.
shader_type spatial;
render_mode blend_mix, depth_prepass_alpha, cull_back, diffuse_burley, specular_schlick_ggx;

/* Multiline comments preserve their rule stack.
   Keywords such as if, uniform, and vec4 stay comments here. */
struct MaterialLayer {
    highp vec4 color;
    mediump vec3 normal;
    lowp float weight;
    int channels[4];
};

struct LightingState {
    vec3 diffuse_light;
    vec3 specular_light;
    bool enabled;
};

uniform sampler2D albedo_map : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D normal_map : hint_normal, filter_linear_mipmap, repeat_disable;
uniform sampler2D roughness_map : hint_roughness_r, filter_nearest_mipmap;
uniform sampler2DArray layer_map : source_color, filter_linear, repeat_enable;
uniform sampler3D volume_map : filter_nearest;
uniform samplerCube environment_map : filter_linear_mipmap_anisotropic;
uniform isampler2D integer_map : filter_nearest;
uniform usampler2D unsigned_map : filter_nearest_mipmap;
global uniform vec4 base_tint : hint_color;
instance uniform vec4 detail_tint : hint_albedo;
uniform vec4 white_level : hint_default_white;
uniform vec4 black_level : hint_default_black;
uniform vec4 black_albedo : hint_black_albedo;
uniform float roughness : hint_range(0.0, 1.0, 0.01);
uniform float anisotropy : hint_anisotropy;
uniform float anisotropy_alt : hint_aniso;
uniform float roughness_g : hint_roughness_g;
uniform vec3 normal_strength : hint_roughness_normal;

varying flat vec3 facet_normal;
varying smooth vec2 detail_uv;
const float SMALL = 1.0e-5;
const float LARGE = 2E+3F;
const uint MASK = 0xDEADu;
const int OCTAVES = 4;
bool enabled = true;
bool hidden = false;
bvec2 decisions = bvec2(true, false);
bvec3 gates = bvec3(true, false, true);
bvec4 lanes = bvec4(false, true, false, true);
ivec2 cell = ivec2(1, 2);
ivec3 offset = ivec3(-1, 0, 1);
ivec4 bounds = ivec4(0, 1, 2, 3);
uvec2 bits2 = uvec2(0x10, 20u);
uvec3 bits3 = uvec3(1u, 2u, 3u);
uvec4 bits4 = uvec4(4u, 5u, 6u, 7u);
mat2 rotation2 = mat2(1.0, 0.0, 0.0, 1.0);
mat3 rotation3 = mat3(1.0);
mat4 transform4 = mat4(1.0);

float saturate_value(float value) {
    return clamp(value, 0.0, 1.0);
}

vec3 decode_normal(in vec3 encoded, out bool valid) {
    vec3 decoded = encoded * 2.0 - 1.0;
    valid = dot(decoded, decoded) > SMALL && enabled;
    return valid ? normalize(decoded) : vec3(0.0, 0.0, 1.0);
}

float hash21(vec2 point) {
    vec3 p3 = fract(vec3(point.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float layered_noise(vec2 point) {
    float sum = 0.0;
    float amplitude = 0.5;
    for (int octave = 0; octave < OCTAVES; octave += 1) {
        if (amplitude < SMALL) { break; }
        sum += hash21(point) * amplitude;
        point = rotation2 * point * 2.03;
        amplitude *= 0.5;
        if (octave == 1) { continue; }
    }
    return sum;
}

vec4 blend_layers(vec4 under, vec4 over, float opacity) {
    float alpha = saturate_value(over.a * opacity);
    vec3 rgb = mix(under.rgb, over.rgb, alpha);
    return vec4(rgb, max(under.a, alpha));
}

MaterialLayer make_layer(vec2 uv, int layer_index) {
    MaterialLayer layer = MaterialLayer();
    vec4 texel = texture(layer_map, vec3(uv, float(layer_index)));
    layer.color = texel * detail_tint;
    layer.normal = texture(normal_map, uv).xyz;
    layer.weight = texel.a;
    layer.channels[0] = layer_index;
    return layer;
}

float classify_band(float value) {
    float band = 0.0;
    switch (int(value * 4.0)) {
        case 0: band = 0.125; break;
        case 1: band = 0.375; break;
        case 2: band = 0.625; break;
        default: band = 0.875; break;
    }
    return band;
}

float converge(float initial) {
    float value = initial;
    int attempts = 0;
    do {
        value = (value + 2.0 / value) * 0.5;
        attempts += 1;
    } while (abs(value * value - 2.0) > SMALL && attempts < 8);
    return value;
}

CustomMaterial build_custom(vec3 color) {
    CustomMaterial result = CustomMaterial();
    result.albedo = color;
    result.roughness = roughness;
    return result;
}

void vertex() {
    vec4 local_position = vec4(VERTEX, 1.0);
    vec4 world_position = MODEL_MATRIX * local_position;
    float wave = sin(TIME + world_position.x * TAU);
    VERTEX.y += wave * 0.025;
    NORMAL = normalize((MODEL_NORMAL_MATRIX * NORMAL).xyz);
    facet_normal = NORMAL;
    detail_uv = UV * 4.0;
}

void fragment() {
    vec4 base_sample = texture(albedo_map, UV);
    MaterialLayer detail = make_layer(detail_uv, 1);
    bool normal_valid = false;
    vec3 mapped_normal = decode_normal(detail.normal, normal_valid);
    vec4 composed = blend_layers(base_sample * base_tint, detail.color, detail.weight);
    float noise_value = layered_noise(UV * 8.0);
    float band = classify_band(noise_value);
    if (!normal_valid || composed.a < 0.001) {
        discard;
    } else if (hidden) {
        ALPHA = 0.0;
    } else {
        ALBEDO = composed.rgb;
        ALPHA = composed.a;
    }
    NORMAL_MAP = mapped_normal * 0.5 + 0.5;
    ROUGHNESS = saturate_value(roughness + band / LARGE);
    METALLIC = float((MASK >> 2u) & 1u);
    EMISSION = base_tint.bgr * E * 0.001;
}

void light() {
    float lambert = max(dot(NORMAL, LIGHT), 0.0);
    float facing = dot(VIEW, NORMAL);
    DIFFUSE_LIGHT += ALBEDO * LIGHT_COLOR * lambert * ATTENUATION;
    SPECULAR_LIGHT += vec3(pow(max(facing, 0.0), 16.0));
}

void start() {
    enabled = PI < TAU;
}

void process() {
    cell <<= 1;
    cell >>= 1;
}

void sky() {
    vec3 direction = EYEDIR.xyz;
    COLOR = texture(environment_map, direction).rgb;
}

void fog() {
    float density = texture(volume_map, WORLD_POSITION).r;
    DENSITY = density != 0.0 ? density : SMALL;
}

// Operators and access forms: .rgba, .stpq, <= >= == != += -= *= /= %= &= |= ^= ~.
// End of stress fixture; Unicode remains inert in comments: 東京 λ 🚀 𝌆.
