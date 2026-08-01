#include <metal_stdlib>
using namespace metal;

struct VertexOut {
    float4 position [[position]];
    float2 uv;
};

vertex VertexOut vertex_main(uint id [[vertex_id]]) {
    VertexOut out;
    out.position = float4(float2(id & 1, id >> 1) * 2.0 - 1.0, 0.0, 1.0);
    out.uv = out.position.xy * 0.5 + 0.5;
    return out;
}

fragment float4 fragment_main(VertexOut in [[stage_in]]) {
    return float4(in.uv, 0.5, 1.0);
}
