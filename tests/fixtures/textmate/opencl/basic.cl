__kernel void scale(__global float *values, const float factor) {
    const size_t index = get_global_id(0);
    values[index] *= factor;
}

float clamp_unit(float value) {
    return clamp(value, 0.0f, 1.0f);
}

__kernel void normalize(__global float4 *values) {
    size_t index = get_global_id(0);
    values[index] = clamp(values[index], (float4)(0.0f), (float4)(1.0f));
}
