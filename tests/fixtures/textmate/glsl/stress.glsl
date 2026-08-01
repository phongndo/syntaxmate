#version 330 core
#extension GL_ARB_explicit_attrib_location : enable
// Grammar stress shader: café, 東京, λ, 🚀, 𝌆.
/* Multiline state:
 * vertex and fragment vocabulary share this oracle fixture.
 */
#define PI 3.14159265358979323846
#define TAU (2.0 * PI)
#define SATURATE(value) clamp((value), 0.0, 1.0)
#define LUMINANCE(rgb) dot((rgb), vec3(0.2126, 0.7152, 0.0722))
#define APPLY_BIAS(value, bias) \
    ((value) + (bias))
#if defined(GL_ES)
precision highp float;
precision mediump int;
#else
#define LOWP
#endif

layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec3 vertexNormal;
layout(location = 2) in vec4 vertexTangent;
layout(location = 3) in vec2 vertexUv;
centroid in vec2 centroidUv;
noperspective in vec3 edgeDistance;
flat in int materialIndex;
smooth out vec3 smoothNormal;
invariant gl_Position;

attribute vec4 legacyVertex;
varying vec2 legacyUv;
uniform mat4 modelMatrix;
uniform mat4 viewMatrix;
uniform mat4 projectionMatrix;
uniform mat3 normalMatrix;
uniform sampler1D rampMap;
uniform sampler2D albedoMap;
uniform sampler3D volumeMap;
uniform samplerCube environmentMap;
uniform sampler2DShadow shadowMap;
uniform sampler2DArray layerMap;
uniform isampler2D integerMap;
uniform usampler2D unsignedMap;
uniform samplerBuffer coefficientBuffer;
uniform sampler2DMS multisampleMap;

const bool lightingEnabled = true;
const int maximumLights = 4;
const uint layerMask = 0x0Fu;
const float epsilon = 1.0e-5;
const vec2 texelScale = vec2(0.5, 0.25);
const vec3 lumaWeights = vec3(0.2126, 0.7152, 0.0722);
const vec4 clearColor = vec4(0.02, 0.03, 0.05, 1.0);
const ivec2 tileOffset = ivec2(1, -2);
const ivec3 voxelOffset = ivec3(1, 2, 3);
const uvec2 tileExtent = uvec2(16u, 16u);
const uvec3 volumeExtent = uvec3(32u, 16u, 8u);
const bvec2 channelPair = bvec2(true, false);
const bvec3 channelMask = bvec3(true, true, false);
const bvec4 writeMask = bvec4(true);

struct Material {
    vec3 baseColor;
    float roughness;
    float metallic;
};

struct Light {
    vec3 position;
    vec3 color;
    float intensity;
};

uniform Material materials[8];
uniform Light lights[maximumLights];

float remap(float value, float low, float high) {
    return SATURATE((value - low) / (high - low));
}

vec3 decodeNormal(vec3 encoded) {
    return normalize(encoded * 2.0 - 1.0);
}

mat3 tangentFrame(vec3 normal, vec4 tangent) {
    vec3 bitangent = cross(normal, tangent.xyz) * tangent.w;
    return mat3(tangent.xyz, bitangent, normal);
}

float distribution(float normalDotHalf, float roughness) {
    float alpha = roughness * roughness;
    float alpha2 = alpha * alpha;
    float denominator = normalDotHalf * normalDotHalf * (alpha2 - 1.0) + 1.0;
    return alpha2 / max(PI * denominator * denominator, epsilon);
}

vec3 fresnel(vec3 base, float viewDotHalf) {
    float factor = pow(1.0 - viewDotHalf, 5.0);
    return mix(base, vec3(1.0), factor);
}

vec3 sampleEnvironment(vec3 direction, float amount) {
    vec3 reflected = reflect(-direction, vec3(0.0, 1.0, 0.0));
    vec3 refracted = refract(-direction, vec3(0.0, 1.0, 0.0), 0.75);
    return mix(textureCube(environmentMap, reflected).rgb,
               textureCube(environmentMap, refracted).rgb, amount);
}

float filteredShadow(vec3 coordinate) {
    float left = shadow2D(shadowMap, coordinate + vec3(-0.001, 0.0, 0.0)).r;
    float right = shadow2D(shadowMap, coordinate + vec3(0.001, 0.0, 0.0)).r;
    return (left + right) * 0.5;
}

vec3 evaluateLight(Material material, Light light, vec3 normal, vec3 viewDirection) {
    vec3 lightDirection = normalize(light.position);
    vec3 halfDirection = normalize(lightDirection + viewDirection);
    float normalDotLight = max(dot(normal, lightDirection), 0.0);
    float normalDotHalf = max(dot(normal, halfDirection), 0.0);
    float viewDotHalf = max(dot(viewDirection, halfDirection), 0.0);
    vec3 dielectric = vec3(0.04);
    vec3 f0 = mix(dielectric, material.baseColor, material.metallic);
    vec3 specular = fresnel(f0, viewDotHalf)
                  * distribution(normalDotHalf, material.roughness);
    vec3 diffuse = material.baseColor * (1.0 - material.metallic);
    return (diffuse + specular) * light.color * light.intensity * normalDotLight;
}

vec4 composite(vec2 uv, vec3 normal) {
    vec4 albedo = texture2D(albedoMap, uv);
    if (albedo.a < epsilon) {
        discard;
    } else if (!lightingEnabled) {
        return albedo;
    }

    Material material = materials[materialIndex];
    material.baseColor *= albedo.rgb;
    vec3 result = vec3(0.0);
    int activeLights = min(maximumLights, gl_MaxLights);
    for (int index = 0; index < activeLights; ++index) {
        if (lights[index].intensity <= 0.0) continue;
        result += evaluateLight(material, lights[index], normal, vec3(0.0, 0.0, 1.0));
    }

    int mode = materialIndex & 3;
    switch (mode) {
        case 0:
            result = floor(result * 8.0) / 8.0;
            break;
        case 1:
            result = ceil(result * 4.0) / 4.0;
            break;
        case 2:
            result = smoothstep(vec3(0.0), vec3(1.0), result);
            break;
        default:
            result = clamp(result, 0.0, 1.0);
            break;
    }

    int samples = 0;
    do {
        result += 0.001 * noise3(vec3(uv, float(samples)));
        ++samples;
    } while (samples < 2);

    while (any(greaterThan(result, vec3(1.0)))) {
        result *= 0.5;
    }
    return vec4(result, albedo.a);
}

void grammarBuiltinProbe(vec3 a, vec3 b, mat3 matrixValue) {
    float angle = radians(45.0) + degrees(PI * 0.25);
    float wave = sin(angle) + cos(angle) + tan(angle);
    float arc = asin(0.5) + acos(0.5) + atan(1.0);
    float exponential = exp2(2.0) + log2(8.0) + inversesqrt(4.0);
    float shaped = abs(sign(wave)) + sqrt(arc) + fract(exponential);
    float metric = length(a) + distance(a, b) + dot(a, b);
    vec3 faced = faceforward(a, b, normalize(a + b));
    bvec3 relations = lessThan(a, b) || greaterThanEqual(a, b);
    bool agreement = all(equal(a, b)) || any(notEqual(a, b));
    mat3 product = matrixCompMult(matrixValue, transpose(matrixValue));
    vec3 selected = agreement ? min(a, b) : max(a, b);
    gl_FragColor = vec4(selected + faced * shaped + vec3(metric), 1.0);
}

void main() {
    mat4 modelView = viewMatrix * modelMatrix;
    mat4 transform = projectionMatrix * modelView;
    vec4 worldPosition = modelMatrix * vec4(vertexPosition, 1.0);
    vec3 transformedNormal = normalize(normalMatrix * vertexNormal);
    mat3 frame = tangentFrame(transformedNormal, vertexTangent);
    vec3 mappedNormal = decodeNormal(texture2D(albedoMap, vertexUv).xyz);
    smoothNormal = normalize(frame * mappedNormal);
    legacyUv = vertexUv * texelScale;
    gl_Position = transform * vec4(vertexPosition, 1.0);
    gl_PointSize = max(1.0, 4.0 / gl_Position.w);
    gl_FragColor = composite(vertexUv, smoothNormal);
    gl_FragDepth = SATURATE(gl_FragCoord.z);
}

// Intentional lexer probes for grammar-declared illegal GLSL words follow.
#if 0
extern inline double forbiddenProbe(unsigned long value);
typedef union { short first; volatile int second; } ForbiddenWords;
goto unreachable_label;
#endif
