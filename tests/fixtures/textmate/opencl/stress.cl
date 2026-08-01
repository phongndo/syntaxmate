#pragma OPENCL EXTENSION cl_khr_fp64 : enable
#define TILE_SIZE 16

constant sampler_t catalog_sampler = CLK_NORMALIZED_COORDS_FALSE | CLK_ADDRESS_CLAMP | CLK_FILTER_NEAREST;

inline float saturate_value(float value) {
    return clamp(value, 0.0f, 1.0f);
}

kernel void tiled_scale(global float *restrict values,
                        global const float *restrict weights,
                        local float *tile,
                        const uint count) {
    const size_t global_index = get_global_id(0);
    const size_t local_index = get_local_id(0);
    tile[local_index] = global_index < count ? values[global_index] : 0.0f;
    barrier(CLK_LOCAL_MEM_FENCE);
    if (global_index < count) {
        values[global_index] = saturate_value(tile[local_index] * weights[global_index]);
    }
}

kernel void transform_0(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(0.0f, 1.0f, 2.0f, 1.0f);
    values[index] = mad(values[index], (float4)(1.0f), offset);
}

kernel void transform_1(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(1.0f, 2.0f, 3.0f, 1.0f);
    values[index] = mad(values[index], (float4)(2.0f), offset);
}

kernel void transform_2(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(2.0f, 3.0f, 4.0f, 1.0f);
    values[index] = mad(values[index], (float4)(3.0f), offset);
}

kernel void transform_3(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(3.0f, 4.0f, 5.0f, 1.0f);
    values[index] = mad(values[index], (float4)(4.0f), offset);
}

kernel void transform_4(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(4.0f, 5.0f, 6.0f, 1.0f);
    values[index] = mad(values[index], (float4)(5.0f), offset);
}

kernel void transform_5(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(5.0f, 6.0f, 7.0f, 1.0f);
    values[index] = mad(values[index], (float4)(6.0f), offset);
}

kernel void transform_6(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(6.0f, 7.0f, 8.0f, 1.0f);
    values[index] = mad(values[index], (float4)(7.0f), offset);
}

kernel void transform_7(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(7.0f, 8.0f, 9.0f, 1.0f);
    values[index] = mad(values[index], (float4)(8.0f), offset);
}

kernel void transform_8(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(8.0f, 9.0f, 10.0f, 1.0f);
    values[index] = mad(values[index], (float4)(9.0f), offset);
}

kernel void transform_9(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(9.0f, 10.0f, 11.0f, 1.0f);
    values[index] = mad(values[index], (float4)(10.0f), offset);
}

kernel void transform_10(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(10.0f, 11.0f, 12.0f, 1.0f);
    values[index] = mad(values[index], (float4)(11.0f), offset);
}

kernel void transform_11(global float4 *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index >= count) return;
    const float4 offset = (float4)(11.0f, 12.0f, 13.0f, 1.0f);
    values[index] = mad(values[index], (float4)(12.0f), offset);
}

kernel void image_copy(read_only image2d_t source,
                       write_only image2d_t destination) {
    const int2 coordinate = (int2)(get_global_id(0), get_global_id(1));
    const float4 pixel = read_imagef(source, catalog_sampler, coordinate);
    write_imagef(destination, coordinate, pixel);
}

kernel void atomic_count(global atomic_uint *counter, global const int *flags) {
    const size_t index = get_global_id(0);
    if (flags[index] != 0) {
        atomic_fetch_add_explicit(counter, 1u, memory_order_relaxed, memory_scope_device);
    }
}

kernel void fill_indices(global uint *values, const uint count) {
    const size_t index = get_global_id(0);
    if (index < count) values[index] = (uint)index;
}

kernel void add_bias(global float *values, const float bias, const uint count) {
    const size_t index = get_global_id(0);
    if (index < count) values[index] += bias;
}

kernel void copy_positive(global const float *source, global float *target, const uint count) {
    const size_t index = get_global_id(0);
    if (index < count && source[index] > 0.0f) target[index] = source[index];
}

// Vector aliases and address-space qualifiers remain lexical coverage.
typedef struct CatalogPair {
    float left;
    float right;
} CatalogPair;
