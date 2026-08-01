#include <metal_stdlib>
#include <metal_atomic>
using namespace metal;

struct VertexInput {
    packed_float3 position;
    packed_float3 normal;
    float2 uv;
};

struct VertexOutput {
    float4 position [[position]];
    float3 normal;
    float2 uv;
};

struct Uniforms {
    float4x4 model_view_projection;
    float4 tint;
    uint item_count;
};

vertex VertexOutput catalog_vertex(const device VertexInput *vertices [[buffer(0)]],
                                   constant Uniforms &uniforms [[buffer(1)]],
                                   uint vertex_id [[vertex_id]]) {
    VertexOutput out;
    VertexInput input = vertices[vertex_id];
    out.position = uniforms.model_view_projection * float4(float3(input.position), 1.0);
    out.normal = normalize(float3(input.normal));
    out.uv = input.uv;
    return out;
}

fragment float4 catalog_fragment(VertexOutput in [[stage_in]],
                                 texture2d<float> color_texture [[texture(0)]],
                                 sampler color_sampler [[sampler(0)]],
                                 constant Uniforms &uniforms [[buffer(1)]]) {
    float4 sampled = color_texture.sample(color_sampler, in.uv);
    float lighting = saturate(dot(in.normal, normalize(float3(0.3, 0.6, 0.7))));
    return sampled * uniforms.tint * float4(lighting, lighting, lighting, 1.0);
}

kernel void transform_0(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(1.0), float4(0.25));
}

kernel void transform_1(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(2.0), float4(1.25));
}

kernel void transform_2(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(3.0), float4(2.25));
}

kernel void transform_3(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(4.0), float4(3.25));
}

kernel void transform_4(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(5.0), float4(4.25));
}

kernel void transform_5(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(6.0), float4(5.25));
}

kernel void transform_6(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(7.0), float4(6.25));
}

kernel void transform_7(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(8.0), float4(7.25));
}

kernel void transform_8(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(9.0), float4(8.25));
}

kernel void transform_9(device float4 *values [[buffer(0)]],
                        constant uint &count [[buffer(1)]],
                        uint index [[thread_position_in_grid]]) {
    if (index >= count) return;
    values[index] = fma(values[index], float4(10.0), float4(9.25));
}

kernel void histogram(texture2d<float, access::read> source [[texture(0)]],
                      device atomic_uint *bins [[buffer(0)]],
                      uint2 coordinate [[thread_position_in_grid]]) {
    if (coordinate.x >= source.get_width() || coordinate.y >= source.get_height()) return;
    float4 pixel = source.read(coordinate);
    uint bin = min(uint(pixel.r * 255.0), 255u);
    atomic_fetch_add_explicit(&bins[bin], 1u, memory_order_relaxed);
}

kernel void fill_indices(device uint *values [[buffer(0)]],
                         uint index [[thread_position_in_grid]]) {
    values[index] = index;
}

kernel void add_bias(device float *values [[buffer(0)]],
                     constant float &bias [[buffer(1)]],
                     uint index [[thread_position_in_grid]]) {
    values[index] += bias;
}

kernel void copy_positive(const device float *source [[buffer(0)]],
                          device float *target [[buffer(1)]],
                          uint index [[thread_position_in_grid]]) {
    if (source[index] > 0.0f) target[index] = source[index];
}

// End Metal fixture.
template<typename T> T identity_value(T value) { return value; }
