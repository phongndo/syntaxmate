#version 330 core
// Unicode shader label: 東京 λ 🚀 𝌆
#define SATURATE(v) clamp((v), 0.0, 1.0)

layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexUv;
uniform sampler2D albedoMap;
uniform mat4 modelViewProjection;
out vec2 fragmentUv;

struct Light { vec3 direction; vec3 color; };
const float gammaValue = 2.2;

vec3 shade(vec3 normal, Light light) {
    float diffuse = max(dot(normalize(normal), -light.direction), 0.0);
    return mix(vec3(0.05), light.color, SATURATE(diffuse));
}

void main() {
    vec4 texel = texture2D(albedoMap, vertexUv);
    if (texel.a <= 0.01) discard;
    fragmentUv = vertexUv;
    gl_Position = modelViewProjection * vec4(vertexPosition, 1.0);
}
