/**
 * Deferred lighting and simulation fixture.
 * BMP text: café, Ελληνικά, 日本語. Astral text: 🚀 and 𝌆.
 * Nested comment coverage: /* all multiline state closes here */
 */
enable f16;

struct Camera {
  view_projection: mat4x4f,
  inverse_view: mat4x4f,
  eye: vec3f,
  exposure: f32,
}

struct Material {
  base_color: vec4f,
  emissive: vec3f,
  roughness: f32,
  metallic: f32,
  enabled: bool,
}

struct VertexInput {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @builtin(instance_index) instance: u32,
}

struct VertexOutput {
  @builtin(position) clip_position: vec4f,
  @location(0) world_position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) @interpolate(flat) material_id: u32,
}

struct Particle {
  position: vec4f,
  velocity: vec4f,
}

type LightIndex = u32;
type WeightTable = array<f32, 4>;

const PI: f32 = 3.14159265;
const HALF: f32 = 0.5;
const NEGATIVE_ONE: i32 = -1;
const HEX_MASK: u32 = 0xFF00u;
const ENABLE_FOG: bool = true;
const DISABLE_DEBUG: bool = false;

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(0) @binding(1)
var<storage, read> materials: array<Material>;

@group(0) @binding(2)
var scene_sampler: sampler;

@group(0) @binding(3)
var albedo_texture: texture_2d<f32>;

@group(1) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(1) @binding(1)
var<storage, read_write> visible_count: atomic<u32>;

var<workgroup> shared_positions: array<vec4f, 64>;
var<private> frame_seed: u32 = 0u;

@id(0)
override workgroup_width: u32 = 8u;

fn saturate(value: f32) -> f32 {
  return clamp(value, 0.0, 1.0);
}

fn square(value: f32) -> f32 {
  return value * value;
}

fn decode_normal(sampled: vec3f) -> vec3f {
  let signed_normal = sampled * 2.0 - vec3f(1.0);
  return normalize(signed_normal);
}

fn distribution_ggx(normal: vec3f, half_vector: vec3f, roughness: f32) -> f32 {
  let alpha = square(roughness);
  let alpha2 = square(alpha);
  let n_dot_h = saturate(dot(normal, half_vector));
  let denominator = square(n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0);
  return alpha2 / max(PI * denominator, 0.0001);
}

fn select_material(index: LightIndex) -> Material {
  let bounded = min(index, arrayLength(&materials) - 1u);
  return materials[bounded];
}

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
  var output: VertexOutput;
  let model_position = vec4f(input.position, 1.0);
  output.clip_position = camera.view_projection * model_position;
  output.world_position = model_position.xyz;
  output.normal = normalize(input.normal);
  output.uv = input.uv;
  output.material_id = input.instance & HEX_MASK;
  return output;
}

fn shade_material(input: VertexOutput, material: Material) -> vec4f {
  let sampled = textureSample(albedo_texture, scene_sampler, input.uv);
  let normal_sample = textureSample(albedo_texture, scene_sampler, input.uv).xyz;
  let mapped_normal = decode_normal(normal_sample);
  let view_direction = normalize(camera.eye - input.world_position);
  let light_direction = normalize(vec3f(0.4, 0.8, 0.2));
  let half_vector = normalize(view_direction + light_direction);
  let diffuse = max(dot(mapped_normal, light_direction), 0.0);
  let specular = distribution_ggx(mapped_normal, half_vector, material.roughness);
  let color = sampled * material.base_color;
  return vec4f(color.rgb * diffuse + material.emissive + specular, color.a);
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4f {
  let material = select_material(input.material_id);
  if (!material.enabled) {
    discard;
  } else if (material.roughness < 0.0) {
    return vec4f(1.0, 0.0, 1.0, 1.0);
  }

  var shaded = shade_material(input, material);
  if (ENABLE_FOG && input.clip_position.z > 10.0) {
    let fog = saturate((input.clip_position.z - 10.0) / 90.0);
    shaded = mix(shaded, vec4f(0.5, 0.6, 0.7, 1.0), fog);
  }

  switch input.material_id {
    case 0u: {
      shaded.rgb += vec3f(0.02, 0.0, 0.0);
    }
    case 1u, 2u: {
      shaded.rgb *= vec3f(0.95, 1.0, 0.95);
      fallthrough;
    }
    default: {
      shaded.rgb = max(shaded.rgb, vec3f(0.0));
    }
  }
  return vec4f(shaded.rgb * camera.exposure, shaded.a);
}

fn integrate_particle(index: u32, delta_time: f32) {
  var particle = particles[index];
  let gravity = vec3f(0.0, -9.81, 0.0);
  particle.velocity.xyz += gravity * delta_time;
  particle.position.xyz += particle.velocity.xyz * delta_time;

  if (particle.position.y < 0.0) {
    particle.position.y = 0.0;
    particle.velocity.y *= -0.65;
  }
  particles[index] = particle;
}

@compute @workgroup_size(8, 8, 1)
fn simulate(@builtin(global_invocation_id) global_id: vec3u,
            @builtin(local_invocation_index) local_index: u32) {
  let index = global_id.x + global_id.y * workgroup_width;
  if (index >= arrayLength(&particles)) {
    return;
  }

  shared_positions[local_index] = particles[index].position;
  workgroupBarrier();
  integrate_particle(index, 0.016);

  var probe: u32 = 0u;
  for (var lane: u32 = 0u; lane < 4u; lane += 1u) {
    probe ^= lane << 1u;
    if (lane == 2u) {
      continue;
    }
    frame_seed += probe;
  }

  while (probe > 8u) {
    probe >>= 1u;
  }

  loop {
    probe += 1u;
    if (probe >= 12u) {
      break;
    }
    continuing {
      frame_seed = frame_seed | probe;
    }
  }

  let prior = atomicAdd(&visible_count, 1u);
  if (DISABLE_DEBUG || prior == 0u) {
    return;
  }
}

/* Final closed block comment — résumé, 宇宙, 🚀, 𝌆. */
