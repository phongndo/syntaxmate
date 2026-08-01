#include <cuda_runtime.h>
#include <cstdio>

__global__ void scale(float *values, float factor, int count) {
    int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index < count) {
        values[index] *= factor;
    }
}

int main() {
    float *device_values = nullptr;
    cudaMalloc(&device_values, 256 * sizeof(float));
    scale<<<1, 256>>>(device_values, 2.0f, 256);
    cudaDeviceSynchronize();
    cudaFree(device_values);
    return 0;
}
