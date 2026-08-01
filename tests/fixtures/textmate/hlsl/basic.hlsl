#define SATURATE(v) clamp((v), 0.0f, 1.0f)
/* Compact HLSL fixture.
 * Unicode: café, λ, 東京, 🚀, 𝌆.
 */
cbuffer Camera : register(b0) {
    row_major float4x4 viewProjection;
    float3 eyePosition; float exposure;
};
Texture2D<float4> albedoMap : register(t0);
SamplerState linearSampler : register(s0);
RWTexture2D<float4> outputMap : register(u0);
static const string shaderLabel = "café λ 東京 🚀 𝌆\n";

struct VertexInput { float3 position : POSITION; float2 uv : TEXCOORD0; };
struct VertexOutput { float4 position : SV_Position; float2 uv : TEXCOORD0; };

VertexOutput VSMain(VertexInput input) {
    VertexOutput output;
    output.position = mul(float4(input.position, 1.0f), viewProjection);
    output.uv = input.uv;
    return output;
}

float4 PSMain(VertexOutput input) : SV_Target0 {
    float4 color = albedoMap.Sample(linearSampler, input.uv);
    if (!color.a) discard;
    return float4(SATURATE(color.rgb * exposure), color.a);
}
