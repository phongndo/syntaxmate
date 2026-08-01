Shader "Mark/GrammarCoverage" {
    // ShaderLab stress fixture: café, naïve, λ, Ω, 東京, 雪; 🚀, 🧪, 𝌆.
    Properties {
        [Header(Surface)] _Label ("Surface café 🚀", Float) = 1
        [HDR] _Color ("Emission Color", Color) = (1, 0.5, 0.25, 1)
        [Gamma] _Gain ("Linear Gain", Float) = 1.25
        [Range(0, 8)] _Smoothness ("Smoothness", Range(0, 8)) = 4
        [IntRange] _Steps ("Steps", Range(1, 16)) = 8
        [Toggle(_ALPHATEST_ON)] _Cutout ("Alpha Test", Float) = 0
        [Enum(UnityEngine.Rendering.CullMode)] _Cull ("Cull Mode", Float) = 2
        [MainTexture] _MainTex ("Albedo", 2D) = "white" {}
        [NoScaleOffset] _DetailTex ("Detail", 2D) = "gray" {}
        [Normal] _NormalMap ("Normal", 2D) = "bump" {}
        [PerRendererData] _MaskTex ("Mask", 2D) = "black" {}
        _VolumeTex ("Volume", 3D) = "white" {}
        _Reflection ("Reflection", Cube) = "black" {}
        _Direction ("Direction", Vector) = (0, 1, 0, 0)
        _Layer ("Layer", Int) = 3
        _Legacy ("Legacy", Any) = "" {}
    }

    Category {
        Tags { "RenderType"="Opaque" "Queue"="Geometry+1" "Projector"="False" }
        LOD 350
        Cull Back
        ZWrite On
        ZTest LEqual
        Offset 0, 0

        SubShader {
            Tags { "RenderPipeline"="UniversalPipeline" "IgnoreProjector"="True" }
            Pass {
                Name "FORWARD_LIT"
                Tags { "LightMode"="UniversalForward" }
                Blend SrcAlpha OneMinusSrcAlpha, One OneMinusSrcAlpha
                BlendOp Add, Max
                ColorMask RGBA
                AlphaToMask Off
                ZWrite On
                ZTest GEqual
                Cull Front
                Offset -1, -1
                Stencil {
                    Ref 2
                    ReadMask 255
                    WriteMask 127
                    Comp Equal
                    CompFront LEqual
                    CompBack NotEqual
                    Pass Replace
                    Fail Keep
                    ZFail IncrSat
                }

                HLSLPROGRAM
                #pragma target 4.5
                #pragma vertex Vert
                #pragma fragment Frag
                #pragma multi_compile _ SHADER_API_D3D11 SHADER_API_METAL
                #define MARK_FEATURE 1
                /* Embedded HLSL block with BMP 雪 and astral 🧪. */
                static const string kLabel = "café λ 東京 🚀 𝌆\n";
                cbuffer MaterialData : register(b0) {
                    row_major float4x4 _Model;
                    float4 _Color;
                    half _Roughness;
                    int _Mode;
                };
                Texture2D<float4> _MainTex : register(t0);
                TextureCube<float4> _CubeTex : register(t1);
                SamplerState sampler_MainTex : register(s0);
                RWTexture2D<float4> _Output : register(u0);
                struct Attributes {
                    float3 positionOS : POSITION;
                    float3 normalOS : NORMAL;
                    float4 tangentOS : TANGENT;
                    float2 uv : TEXCOORD0;
                    uint instanceID : SV_InstanceID;
                };
                struct Varyings {
                    float4 positionCS : SV_Position;
                    nointerpolation uint instanceID : TEXCOORD1;
                    centroid float2 uv : TEXCOORD0;
                    float3 normalWS : TEXCOORD2;
                };
                Varyings Vert(Attributes input) {
                    Varyings output;
                    float4 world = mul(_Model, float4(input.positionOS, 1.0f));
                    output.positionCS = mul(UNITY_MATRIX_VP, world);
                    output.normalWS = mul((float3x3)_Model, input.normalOS);
                    output.uv = input.uv;
                    output.instanceID = input.instanceID;
                    return output;
                }
                float4 Frag(Varyings input, bool facing : SV_IsFrontFace) : SV_Target0 {
                    float4 texel = _MainTex.Sample(sampler_MainTex, input.uv);
                    float pulse = _SinTime.w * 0.5f + 0.5f;
                    float cameraDistance = length(_WorldSpaceCameraPos - input.normalWS);
                    if (!facing || texel.a < 0.01f) discard;
                    switch (_Mode) {
                        case 0: texel.rgb *= unity_AmbientSky.rgb; break;
                        case 1: texel.rgb += _LightColor0.rgb * pulse; break;
                        default: texel.rgb = saturate(texel.rgb); break;
                    }
                    for (int i = 0; i < 2; ++i) texel.rgb += 0.01f;
                    return texel * _Color + cameraDistance * 0.0001f;
                }
                #ifdef SHADER_API_D3D11
                float PlatformValue() { return SHADER_TARGET + UNITY_VERSION; }
                #else
                float PlatformValue() { return _Time.y + unity_DeltaTime.x; }
                #endif
                ENDHLSL
            }

            Pass {
                Name "SHADOW_CASTER"
                Tags { "LightMode"="ShadowCaster" }
                ZWrite On
                ZTest Less
                Cull Back
                ColorMask 0
                CGPROGRAM
                #pragma vertex shadowVert
                #pragma fragment shadowFrag
                #pragma multi_compile_shadowcaster
                fixed4 _ShadowTint;
                sampler2D _MainTex;
                struct ShadowInput {
                    float4 vertex : POSITION;
                    float2 texcoord : TEXCOORD0;
                };
                struct ShadowOutput {
                    float4 pos : SV_Position;
                    float2 uv : TEXCOORD0;
                };
                ShadowOutput shadowVert(ShadowInput v) {
                    ShadowOutput o;
                    o.pos = mul(UNITY_MATRIX_MVP, v.vertex);
                    o.uv = v.texcoord;
                    return o;
                }
                fixed4 shadowFrag(ShadowOutput i) : SV_Target {
                    fixed4 sampled = tex2D(_MainTex, i.uv);
                    if (sampled.a <= 0.5) discard;
                    return sampled * _ShadowTint;
                }
                ENDCG
            }

            Pass {
                Name "FIXED_FUNCTION"
                Lighting On
                SeparateSpecular On
                ColorMaterial AmbientAndDiffuse
                AlphaTest Greater 0.25
                Fog { Mode Exp2 Density 0.02 Color (0.2, 0.3, 0.4, 1) }
                Material {
                    Diffuse (1, 1, 1, 1)
                    Ambient (0.2, 0.2, 0.2, 1)
                    Specular (0.8, 0.8, 0.8, 1)
                    Emission (0, 0, 0, 1)
                    Shininess 0.75
                }
                SetTexture [_MainTex] {
                    ConstantColor (1, 1, 1, 1)
                    Matrix [_TextureMatrix]
                    Combine Texture * Primary Double, Texture * Constant
                }
                SetTexture [_DetailTex] {
                    Combine Previous + Texture, Previous * Texture Alpha
                }
                BindChannels {
                    Bind "Vertex", Vertex
                    Bind "normal", Normal
                    Bind "texcoord", TexCoord0
                    Bind "texcoord1", TexCoord1
                    Bind "tangent", Tangent
                }
            }

            GrabPass { "_SceneCopy" }
            UsePass "Legacy Shaders/VertexLit/SHADOWCASTER"
        }
    }

    Dependency "AddPassShader" = "Mark/GrammarCoverageAdd"
    Fallback "VertexLit"
    CustomEditor "Mark.Editor.GrammarCoverageGUI"
}
