Shader "Mark/UnicodeLit" {
    // BMP: café λ 東京; astral: 🚀 𝌆
    Properties {
        [HDR] _Tint ("Tint café λ 東京 🚀 𝌆", Color) = (1, 0.5, 0.25, 1)
        [Range(0, 2)] _Gloss ("Gloss", Range(0, 2)) = 0.75
        [MainTexture] _MainTex ("Albedo", 2D) = "white" {}
    }
    SubShader {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" }
        LOD 200
        Pass {
            Name "FORWARD"
            Cull Back
            ZWrite On
            ZTest LEqual
            Blend SrcAlpha OneMinusSrcAlpha
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            struct Attributes { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct Varyings { float4 positionCS : SV_Position; float2 uv : TEXCOORD0; };
            Varyings vert(Attributes input) {
                Varyings output; output.positionCS = input.positionOS; output.uv = input.uv; return output;
            }
            float4 frag(Varyings input) : SV_Target { return float4(input.uv, 0.0, 1.0) * _Tint; }
            ENDHLSL
        }
    }
    Fallback "Diffuse"
}
