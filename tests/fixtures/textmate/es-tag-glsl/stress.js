const MAX_LIGHTS = 4;
const workgroupSize = 8;
const includeDefines = ["USE_FOG", "USE_SHADOWS"];
const sourceLabel = "café / 東京 / 🚀 / 𝌆";

export const vertexShader = glsl`
  #version 300 es
  #define MAX_LIGHTS ${MAX_LIGHTS}
  #define SATURATE(value) clamp(value, 0.0, 1.0)
  precision highp float;
  precision highp int;
  // Unicode source label supplied by the host: ${sourceLabel}
  /* Vertex stage: café, 東京, 🚀, and 𝌆 remain inside a closed comment. */
  layout(location = 0) in vec3 a_position;
  layout(location = 1) in vec3 a_normal;
  layout(location = 2) in vec2 a_texCoord;
  layout(location = 3) in vec4 a_tangent;
  uniform CameraBlock {
    mat4 view;
    mat4 projection;
    vec3 cameraPosition;
    float exposure;
  } camera;
  uniform mat4 u_model;
  uniform mat3 u_normalMatrix;
  uniform vec4 u_clipPlane;
  out VS_OUT {
    vec3 worldPosition;
    vec3 worldNormal;
    vec2 texCoord;
    mat3 tangentBasis;
    float clipDistance;
  } vertex;
  mat3 makeTangentBasis(vec3 normal, vec4 tangent) {
    vec3 t = normalize(tangent.xyz);
    vec3 n = normalize(normal);
    vec3 b = normalize(cross(n, t)) * tangent.w;
    return mat3(t, b, n);
  }
  float waveOffset(vec3 point) {
    const float tau = 6.28318530718;
    float primary = sin(point.x * tau + point.z * 0.5);
    float secondary = cos(point.z * tau * 0.25);
    return (primary + secondary) * 0.025;
  }
  void main() {
    vec3 displaced = a_position;
    displaced.y += waveOffset(a_position);
    vec4 world = u_model * vec4(displaced, 1.0);
    vec3 normal = normalize(u_normalMatrix * a_normal);
    vertex.worldPosition = world.xyz;
    vertex.worldNormal = normal;
    vertex.texCoord = a_texCoord;
    vertex.tangentBasis = makeTangentBasis(normal, a_tangent);
    vertex.clipDistance = dot(world, u_clipPlane);
    gl_Position = camera.projection * camera.view * world;
    gl_PointSize = max(1.0, 8.0 / max(gl_Position.w, 0.001));
  }
`;

const toneMap = "ACES";
export const fragmentShader = /* inline-glsl */ `
  #version 300 es
  precision highp float;
  precision highp sampler2DShadow;
  ${includeDefines.map((name) => `#define ${name} 1`).join("\n")}
  #define PI 3.141592653589793
  #define EPSILON 1e-5
  struct Material {
    vec3 baseColor;
    float metallic;
    float roughness;
    float occlusion;
  };
  struct Light {
    vec4 positionAndType;
    vec3 color;
    float intensity;
  };
  uniform Material u_material;
  uniform Light u_lights[${MAX_LIGHTS}];
  uniform sampler2D u_albedoMap;
  uniform sampler2D u_normalMap;
  uniform samplerCube u_environment;
  uniform sampler2DShadow u_shadowMap;
  uniform bool u_useNormalMap;
  uniform int u_debugMode;
  uniform float u_time;
  uniform vec3 u_cameraPosition;
  uniform float u_exposure;
  in VS_OUT {
    vec3 worldPosition;
    vec3 worldNormal;
    vec2 texCoord;
    mat3 tangentBasis;
    float clipDistance;
  } fragment;
  layout(location = 0) out vec4 outColor;
  layout(location = 1) out vec4 outBloom;
  vec3 decodeNormal(vec2 uv) {
    vec3 sampleValue = texture(u_normalMap, uv).xyz * 2.0 - 1.0;
    return normalize(fragment.tangentBasis * sampleValue);
  }
  float distributionGGX(vec3 n, vec3 h, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float nDotH = max(dot(n, h), 0.0);
    float denominator = nDotH * nDotH * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denominator * denominator, EPSILON);
  }

  float geometrySchlickGGX(float nDotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return nDotV / max(nDotV * (1.0 - k) + k, EPSILON);
  }

  vec3 fresnelSchlick(float cosine, vec3 f0) {
    return f0 + (1.0 - f0) * pow(1.0 - SATURATE(cosine), 5.0);
  }

  vec3 evaluateLight(Light light, vec3 n, vec3 v, Material material) {
    vec3 delta = light.positionAndType.xyz - fragment.worldPosition;
    vec3 l = light.positionAndType.w == 0.0 ? normalize(-delta) : normalize(delta);
    vec3 h = normalize(v + l);
    float distanceSquared = max(dot(delta, delta), 0.01);
    float attenuation = light.positionAndType.w == 0.0 ? 1.0 : 1.0 / distanceSquared;
    float nDotL = max(dot(n, l), 0.0);
    float nDotV = max(dot(n, v), 0.0);

    vec3 f0 = mix(vec3(0.04), material.baseColor, material.metallic);
    vec3 f = fresnelSchlick(max(dot(h, v), 0.0), f0);
    float d = distributionGGX(n, h, material.roughness);
    float g = geometrySchlickGGX(nDotL, material.roughness)
            * geometrySchlickGGX(nDotV, material.roughness);
    vec3 specular = (d * g * f) / max(4.0 * nDotL * nDotV, EPSILON);
    vec3 diffuse = (1.0 - f) * (1.0 - material.metallic) * material.baseColor / PI;
    return (diffuse + specular) * light.color * light.intensity * attenuation * nDotL;
  }

  vec3 toneMapACES(vec3 color) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), 0.0, 1.0);
  }

  void main() {
    if (fragment.clipDistance < 0.0) discard;

    vec4 albedoSample = texture(u_albedoMap, fragment.texCoord);
    Material material = u_material;
    material.baseColor *= pow(albedoSample.rgb, vec3(2.2));
    material.roughness = clamp(material.roughness, 0.04, 1.0);

    vec3 n = u_useNormalMap ? decodeNormal(fragment.texCoord) : normalize(fragment.worldNormal);
    vec3 v = normalize(u_cameraPosition - fragment.worldPosition);
    vec3 radiance = vec3(0.0);

    for (int i = 0; i < MAX_LIGHTS; ++i) {
      radiance += evaluateLight(u_lights[i], n, v, material);
    }

    vec3 reflected = textureLod(u_environment, reflect(-v, n), material.roughness * 8.0).rgb;
    radiance += reflected * material.occlusion * 0.2;

    switch (u_debugMode) {
      case 1: radiance = n * 0.5 + 0.5; break;
      case 2: radiance = vec3(material.roughness); break;
      case 3: radiance = vec3(fract(fragment.texCoord * 10.0), 0.0); break;
      default: break;
    }

    // Host-selected operator: ${toneMap}; escaped newline text: \n.
    vec3 mapped = toneMapACES(radiance * u_exposure);
    outColor = vec4(pow(mapped, vec3(1.0 / 2.2)), albedoSample.a);
    outBloom = vec4(max(mapped - vec3(1.0), vec3(0.0)), 1.0);
  }
`;

export const computeShader =
  // inline-glsl
  `
    #version 310 es
    layout(local_size_x = ${workgroupSize}, local_size_y = ${workgroupSize}, local_size_z = 1) in;

    layout(std430, binding = 0) readonly buffer InputParticles {
      vec4 positions[];
    } inputParticles;

    layout(std430, binding = 1) buffer OutputParticles {
      vec4 velocities[];
    } outputParticles;

    layout(binding = 0, rgba16f) uniform highp image2D velocityImage;
    uniform uint u_particleCount;
    uniform float u_deltaTime;

    shared vec3 tileVelocity[${workgroupSize * workgroupSize}];

    uint flatten(uvec2 value, uint width) {
      return value.y * width + value.x;
    }

    void main() {
      uvec2 pixel = gl_GlobalInvocationID.xy;
      uint particle = flatten(pixel, gl_NumWorkGroups.x * gl_WorkGroupSize.x);
      uint localIndex = gl_LocalInvocationIndex;

      vec3 velocity = particle < u_particleCount
        ? inputParticles.positions[particle].xyz * u_deltaTime
        : vec3(0.0);
      tileVelocity[localIndex] = velocity;
      barrier();

      if (particle < u_particleCount) {
        vec3 average = tileVelocity[localIndex] * u_deltaTime;
        imageStore(velocityImage, ivec2(pixel), vec4(average, 1.0));
        outputParticles.velocities[particle] = vec4(average, 0.0);
      }

      memoryBarrierBuffer();
    }
  `;

export const shaderCharacters = vertexShader.length + fragmentShader.length + computeShader.length;
