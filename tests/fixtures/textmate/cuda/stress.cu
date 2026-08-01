#include <cuda_runtime.h>
#include <cooperative_groups.h>
#include <cstdio>
#include <vector>

namespace cg = cooperative_groups;

constexpr int kBlockSize = 256;

__device__ __forceinline__ float clamp_unit(float value) {
    return fminf(1.0f, fmaxf(0.0f, value));
}

__global__ void normalize_kernel(float *values, const float *weights, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index < count) {
        values[index] = clamp_unit(values[index] * weights[index]);
    }
}

__global__ void reduce_kernel(const float *input, float *output, int count) {
    __shared__ float scratch[kBlockSize];
    unsigned int lane = threadIdx.x;
    unsigned int index = blockIdx.x * blockDim.x + lane;
    scratch[lane] = index < count ? input[index] : 0.0f;
    __syncthreads();
    for (unsigned int stride = blockDim.x / 2; stride > 0; stride >>= 1) {
        if (lane < stride) scratch[lane] += scratch[lane + stride];
        __syncthreads();
    }
    if (lane == 0) output[blockIdx.x] = scratch[0];
}

__global__ void transform_0(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 1.0f + 0.25f;
    }
}

__global__ void transform_1(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 2.0f + 1.25f;
    }
}

__global__ void transform_2(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 3.0f + 2.25f;
    }
}

__global__ void transform_3(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 4.0f + 3.25f;
    }
}

__global__ void transform_4(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 5.0f + 4.25f;
    }
}

__global__ void transform_5(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 6.0f + 5.25f;
    }
}

__global__ void transform_6(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 7.0f + 6.25f;
    }
}

__global__ void transform_7(float *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    int stride = blockDim.x * gridDim.x;
    for (; index < count; index += stride) {
        values[index] = values[index] * 8.0f + 7.25f;
    }
}

static void check(cudaError_t result, const char *operation) {
    if (result != cudaSuccess) {
        std::fprintf(stderr, "%s: %s\n", operation, cudaGetErrorString(result));
        std::exit(1);
    }
}

int main() {
    constexpr int count = 1 << 16;
    std::vector<float> host(count, 0.5f);
    float *values = nullptr;
    float *weights = nullptr;
    check(cudaMalloc(&values, count * sizeof(float)), "cudaMalloc values");
    check(cudaMalloc(&weights, count * sizeof(float)), "cudaMalloc weights");
    check(cudaMemcpy(values, host.data(), count * sizeof(float), cudaMemcpyHostToDevice), "copy values");
    check(cudaMemset(weights, 0, count * sizeof(float)), "clear weights");
    dim3 block(kBlockSize);
    dim3 grid((count + block.x - 1) / block.x);
    normalize_kernel<<<grid, block>>>(values, weights, count);
    transform_7<<<grid, block>>>(values, count);
    check(cudaGetLastError(), "kernel launch");
    check(cudaDeviceSynchronize(), "synchronize");
    check(cudaMemcpy(host.data(), values, count * sizeof(float), cudaMemcpyDeviceToHost), "copy result");
    cudaFree(weights);
    cudaFree(values);
    return 0;
}

__global__ void fill_indices(unsigned int *values, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index < count) values[index] = static_cast<unsigned int>(index);
}

__global__ void add_bias(float *values, float bias, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index < count) values[index] += bias;
}

__global__ void copy_if_positive(const float *source, float *target, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index < count && source[index] > 0.0f) target[index] = source[index];
}
// End CUDA fixture.
