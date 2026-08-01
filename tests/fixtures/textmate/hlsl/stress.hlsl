/*
 * HLSL TextMate stress fixture: broad, reviewable grammar coverage.
 * Unicode in comments: café, λ, 東京, 🚀, 𝌆.
 */
#define FIXTURE_SCALE 2.0F
#define APPLY_SCALE(v) ((v) * FIXTURE_SCALE)
#ifndef FIXTURE_INCLUDED
#define FIXTURE_INCLUDED 1
#endif
#ifdef UNUSED_FEATURE
#undef UNUSED_FEATURE
#endif
#if 0
#error This inactive branch exercises the error directive.
#elif 1
#pragma warning(disable: 3205)
#else
#line 200 "generated-fixture.hlsl"
#endif
#include "shared-fixture.hlsli"
typedef float4 Color4;
namespace Fixture {
struct MaterialData {
    float4 baseColor;
    uint flags;
};
interface ILighting {
    float3 Evaluate(float3 normal);
};
class LambertLighting : ILighting {
    float3 Evaluate(float3 normal) { return max(normal.z, 0.0f); }
};
}
cbuffer SceneConstants : register(b0) {
    row_major float4x4 viewProjection : packoffset(c0);
    column_major matrix<float, 3, 3> normalMatrix : packoffset(c4);
    float3 cameraPosition; float elapsedSeconds;
};
tbuffer LegacyConstants : register(t15) {
    vector<float, 4> legacyTint;
};
ConstantBuffer<Fixture::MaterialData> materialConstants : register(b1);
extern uniform float externalExposure;
static const dword packedMask = 0xFF00AA55;
volatile uint volatileCounter;
shared half sharedValue;
groupshared uint groupCounts[64];
globallycoherent RWByteAddressBuffer globalBytes : register(u0);
precise float4 precisePosition;
min10float tinyValue = .5F;
min12int smallSigned = 12;
min16float2 smallPair = float2(1.25f, 2.0);
min16int3 smallInts = int3(-1, 0, 1);
min16uint4 smallUints = uint4(1, 2, 3, 4);
double2 highPrecision = double2(1.0, 2.0);
bool4 enabledLanes = bool4(true, false, true, false);
snorm float4 signedNormalized;
unorm float4 unsignedNormalized;
Buffer<float4> typedBuffer : register(t0);
ByteAddressBuffer rawBuffer : register(t1);
AppendStructuredBuffer<float4> appendBuffer : register(u1);
ConsumeStructuredBuffer<float4> consumeBuffer : register(u2);
RWBuffer<uint> writableBuffer : register(u3);
RWStructuredBuffer<float4> writableStructured : register(u4);
RWTexture1D<float> writable1D : register(u5);
RWTexture1DArray<float> writable1DArray : register(u6);
RWTexture2D<float4> writable2D : register(u7);
RWTexture2DArray<float4> writable2DArray : register(u8);
RWTexture3D<float4> writable3D : register(u9);
RasterizerOrderedBuffer<uint> orderedBuffer : register(u10);
RasterizerOrderedTexture2D<float4> orderedTexture : register(u11);
Texture1D<float> texture1D : register(t2);
Texture1DArray<float> texture1DArray : register(t3);
Texture2D<float4> surfaceTexture : register(t4);
Texture2DArray<float4> texture2DArray : register(t5);
Texture2DMS<float4> texture2DMS : register(t6);
Texture2DMSArray<float4> texture2DMSArray : register(t7);
Texture3D<float4> texture3D : register(t8);
TextureCube<float4> textureCube : register(t9);
TextureCubeArray<float4> textureCubeArray : register(t10);
texture2D legacyTexture2D;
textureCUBE legacyTextureCube;
sampler legacySampler;
sampler1D legacySampler1D;
sampler2D legacySampler2D;
sampler3D legacySampler3D;
samplerCUBE legacySamplerCube;
sampler_state legacySamplerState;
SamplerComparisonState shadowSampler : register(s1);
SamplerState surfaceSampler {
    Filter = ANISOTROPIC;
    AddressU = WRAP;
    AddressV = MIRROR;
    AddressW = CLAMP;
    MipLODBias = 0.0f;
    MaxAnisotropy = 8;
    ComparisonFunc = LESS_EQUAL;
    BorderColor = float4(0.0f, 0.0f, 0.0f, 1.0f);
    MinLOD = 0;
    MaxLOD = 16;
};

RasterizerState fixtureRasterizer {
    FillMode = SOLID;
    CullMode = BACK;
    FrontCounterClockwise = FALSE;
    DepthBias = 0;
    DepthBiasClamp = 0.0f;
    SlopeScaleDepthBias = 1.0f;
    ZClipEnable = TRUE;
    ScissorEnable = FALSE;
    MultiSampleEnable = TRUE;
    AntiAliasedLineEnable = FALSE;
};

BlendState fixtureBlend {
    AlphaToCoverageEnable = FALSE;
    BlendEnable[0] = TRUE;
    SrcBlend = SRC_ALPHA;
    DestBlend = INV_SRC_ALPHA;
    BlendOp = ADD;
    SrcBlendAlpha = ONE;
    DestBlendAlpha = ZERO;
    BlendOpAlpha = REV_SUBTRACT;
    RenderTargetWriteMask[0] = ALL;
};

DepthStencilState fixtureDepth {
    DepthEnable = TRUE;
    DepthWriteMask = ALL;
    DepthFunc = GREATER_EQUAL;
    StencilEnable = TRUE;
    StencilReadMask = 0xFF;
    StencilWriteMask = 0x7F;
    FrontFaceStencilFail = KEEP;
    FrontFaceStencilZFail = INCR_SAT;
    FrontFaceStencilPass = REPLACE;
    FrontFaceStencilFunc = ALWAYS;
    BackFaceStencilFail = DECR_SAT;
    BackFaceStencilZFail = INVERT;
    BackFaceStencilPass = DECR;
    BackFaceStencilFunc = NOT_EQUAL;
};

struct VertexInput {
    float3 position : POSITION;
    float3 normal : NORMAL0;
    float4 tangent : TANGENT;
    centroid float2 uv : TEXCOORD0;
    uint instance : SV_InstanceID;
    uint vertex : SV_VertexID;
};
struct VertexOutput {
    float4 position : SV_Position;
    linear noperspective float2 uv : TEXCOORD0;
    nointerpolation uint material : COLOR1;
    sample float coverage : SV_Coverage;
};
struct PixelOutput {
    float4 color : SV_Target0;
    float depth : SV_DepthGreaterEqual;
};
export void WriteColor(out Color4 value) { value = (unsigned)packedMask; }
inline VertexOutput VSMain(in VertexInput input) {
    VertexOutput output;
    output.position = mul(float4(input.position, 1.0f), viewProjection);
    output.uv = input.uv;
    output.material = input.instance;
    output.coverage = 1.0f;
    return output;
}

PixelOutput PSMain(VertexOutput input, bool front : SV_IsFrontFace,
                   uint innerCoverage : SV_InnerCoverage) {
    PixelOutput output;
    float4 sampled = surfaceTexture.Sample(surfaceSampler, input.uv);
    [loop] for (int i = 0; i < 4; ++i) {
        if (i == 1) continue;
        sampled.rgb += i * 0.01f;
    }
    int choice = 2;
    switch (choice) {
        case 0: sampled.rgb = 0.0f; break;
        case 1: sampled.rgb = 1.0f; break;
        default: sampled.rgb = saturate(sampled.rgb); break;
    }
    do { choice--; } while (choice > 0);
    if (!front) sampled.rgb *= .75f; else sampled.rgb *= 1.0f;
    if (sampled.a <= 0.001f) discard;
    output.color = sampled;
    output.depth = 0.5f;
    return output;
}

[numthreads(8, 8, 1)]
void CSMain(uint3 dispatchId : SV_DispatchThreadID,
            uint3 groupId : SV_GroupID,
            uint3 groupThreadId : SV_GroupThreadID,
            uint groupIndex : SV_GroupIndex) {
    uint address = dispatchId.x * 4;
    globalBytes.Store(address, groupIndex + groupId.x + groupThreadId.x);
}

struct HSControlPoint { float3 position : POSITION; };
struct HSPatchData { float edges[3] : SV_TessFactor; float inside : SV_InsideTessFactor; };
HSPatchData PatchConstants(InputPatch<HSControlPoint, 3> patch,
                           uint patchId : SV_PrimitiveID) {
    HSPatchData data = (HSPatchData)0;
    data.edges[0] = data.edges[1] = data.edges[2] = 4.0f;
    data.inside = 4.0f + patchId * 0.0f;
    return data;
}
HSControlPoint HSMain(InputPatch<HSControlPoint, 3> patch,
                      uint pointId : SV_OutputControlPointID,
                      uint primitiveId : SV_PrimitiveID) {
    return patch[pointId];
}
float4 DSMain(HSPatchData data, float3 location : SV_DomainLocation,
              const OutputPatch<HSControlPoint, 3> patch) : SV_Position {
    return float4(location + patch[0].position * data.inside, 1.0f);
}

[maxvertexcount(3)]
void GSMain(triangle VertexOutput inputVertices[3],
            inout TriangleStream<VertexOutput> outputStream,
            uint instanceId : SV_GSInstanceID) {
    for (uint i = 0; i < 3; ++i) outputStream.Append(inputVertices[i]);
    outputStream.RestartStrip();
}
void StreamSignatures(point VertexOutput pointInput[1],
                      inout PointStream<VertexOutput> points,
                      lineadj VertexOutput lineInput[4],
                      inout LineStream<VertexOutput> lines) { return; }
void PrimitiveKinds(line VertexOutput edge[2], triangleadj VertexOutput adjacent[6]) { return; }

string shaderLabel = "café λ 東京 🚀 𝌆 and escaped quote: \"done\"";
BlendState optionalBlend = NULL;

technique LegacyTechnique {
    pass LegacyPass {
        VertexShader = compile vs_5_0 VSMain();
        PixelShader = compile ps_5_0 PSMain();
    }
}
Technique ModernTechnique { pass P0 { PixelShader = NULL; } }
technique10 TenTechnique { pass P0 { ComputeShader = compile cs_5_0 CSMain(); } }
technique11 ElevenTechnique { pass P0 { RasterizerState = fixtureRasterizer; } }
