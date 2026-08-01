; NASM x86-64: café, π, and an astral rocket 🚀
bits 64
default rel
%define EXIT_NR 60
global _start
extern puts
section .rodata
message: db 0x68, 0x69, 10, 0
mask: dq 0xFF00_FF00, 0b1010_0101
section .bss
scratch: resb 16
section .text
_start:
  lea rsi, [message]
  movzx eax, byte [rsi]
  cmp al, 0x68
  jne .fallback
  vpxor xmm0, xmm0, xmm0
  mov rax, EXIT_NR
  xor edi, edi
  syscall
.fallback:
  lock inc qword [scratch]
  mov edi, 2
  jmp _start
