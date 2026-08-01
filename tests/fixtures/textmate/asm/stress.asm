; Streaming checksum and text classifier for x86-64.
; The comments safely carry BMP café/λ and astral symbols 🧭🚀.
bits 64
default rel
%define ABI_SYSV 1
%idefine cache_line 64
%assign MAX_CHUNK (4 * 1024)
%define FEATURE_WORD %eval((1 << 9) | (1 << 19))
%strlen banner_length "assembly telemetry"
%substr banner_initial "assembly telemetry", 1, 1
%ifdef USE_PLATFORM_ABI
%include "platform/abi.inc"
%endif
%ifndef ABI_SYSV
%warning "building without the SysV register map"
%endif
%macro SAVE_CALLEE 0
    push rbp
    mov rbp, rsp
    push rbx
    push r12
%endmacro
%macro RESTORE_CALLEE 0
    pop r12
    pop rbx
    leave
    ret
%endmacro
%if 0
; Retained design sketch: this whole conditional is a closed comment state.
legacy_entry:
    mov eax, 0xDEAD
    int 0x80
%endif
/* The lexer also accepts a C-style block comment.
   This closed note spans lines without hiding the module that follows. */
extern malloc
global checksum_update
global classify_utf8
global probe_features
global dispatch_checksum
section .rodata
align 64
banner:         db "assembly telemetry — café 🛰️", 10, 0
escaped_path:   db `cache\line\n`, 0
printf_shape:   db "crc=%08x, bytes=%zu", 0
binary_masks:   dd 0b1010_0101, 1100_0011b
octal_modes:    dw 0o755, 0644q
decimal_limits: dd 0d65_535, 1_000_000
hex_masks:      dq 0xFF00_FF00_AA55_AA55, 0h7FFF_FFFF, 0BADC0DEh
scalar_floats:  dd 1.25, 6.022e+23, 0x1.8p+2
wide_floats:    dq 0b1.01p+3, 0o7.4p-1, 3.1415926535
packed_bcd:     dt 123456789p
fp_specials:    dq __?float64?__(1.0), __float64__(2.0), Inf, QNaN
build_stamp:    db __?DATE?__, " ", __?TIME?__, 0
source_stamp:   db __FILE__, ':', __?LINE?__, 0
dispatch_slot:  dq checksum_update wrt ..gotpc
align 32
shuffle_bytes:  db 15, 14, 13, 12, 11, 10, 9, 8
                db 7, 6, 5, 4, 3, 2, 1, 0
float_scale:    dd 0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5
aes_round_key:  do 0x00112233445566778899AABBCCDDEEFF
vector_seed:    dy 1, 2, 3, 4, 5, 6, 7, 8
zmm_seed:       dz 16 dup (0x3F800000)
section .data
align 8
selected_impl:  dq checksum_scalar
feature_cache:  dd FEATURE_WORD
section .bss
alignb cache_line
scratch_byte:   resb 1
scratch_words:  resw 8
scratch_dwords: resd 16
scratch_qwords: resq 8
scratch_oword:  reso 1
scratch_ymm:    resy 1
scratch_zmm:    resz 1
fpu_temp:       rest 1
section .text
align 16
checksum_update:
    SAVE_CALLEE
    mov r12, rdi
    mov rbx, rsi
    test rsi, rsi
    jz .empty
    mov eax, edx
.byte_loop:
    movzx ecx, byte [r12]
    crc32 eax, cl
    rol eax, 5
    xor eax, ecx
    inc r12
    dec rbx
    jnz .byte_loop
    lock xadd dword [rel feature_cache], eax
    bswap eax
    jmp .done
.empty:
    xor eax, eax
.done:
    RESTORE_CALLEE
align 16
checksum_scalar:
    xor eax, eax
    xor ecx, ecx
.next_word:
    cmp rsi, 8
    jb .tail
    mov rdx, qword [rdi]
    add rax, rdx
    rorx r8, rdx, 17
    adcx rax, r8
    add rdi, 8
    sub rsi, 8
    jmp .next_word
.tail:
    test rsi, rsi
    jz .return
    movzx edx, byte [rdi]
    imul eax, eax, 33
    add eax, edx
    inc rdi
    dec rsi
    jmp .tail
.return:
    ret
align 16
classify_utf8:
    xor eax, eax
    xor edx, edx
.scan:
    cmp rdx, rsi
    jae .classified
    movzx ecx, byte [rdi + rdx]
    test cl, 0x80
    setnz r8b
    add al, r8b
    inc rdx
    jmp .scan
.classified:
    ret
align 16
copy_forward:
    mov rcx, rdx
    cld
    rep movsb
    ret
align 16
checksum_sse2:
    pxor xmm0, xmm0
.sse_loop:
    cmp rsi, 16
    jb .sse_reduce
    movdqu xmm1, oword [rdi]
    paddd xmm0, xmm1
    pshufb xmm0, [rel shuffle_bytes]
    add rdi, 16
    sub rsi, 16
    jmp .sse_loop
.sse_reduce:
    ptest xmm0, xmm0
    movd eax, xmm0
    ret

align 32
checksum_avx2:
    vpxor ymm0, ymm0, ymm0
    vbroadcastss ymm2, [rel float_scale]
.avx_loop:
    cmp rsi, 32
    jb .avx_reduce
    vmovdqu ymm1, yword [rdi]
    vpaddd ymm0, ymm0, ymm1
    vpermq ymm0, ymm0, 0b01_00_11_10
    vfmadd231ps ymm3, ymm2, ymm1
    add rdi, 32
    sub rsi, 32
    jmp .avx_loop
.avx_reduce:
    vextracti128 xmm1, ymm0, 1
    vpaddd xmm0, xmm0, xmm1
    vmovd eax, xmm0
    vzeroupper
    ret

align 16
mix_aes_sha:
    movdqu xmm0, [rdi]
    movdqa xmm1, [rel aes_round_key]
    aesenc xmm0, xmm1
    aesdeclast xmm0, xmm1
    pclmulqdq xmm0, xmm1, 0x11
    sha1msg1 xmm0, xmm1
    sha256rnds2 xmm0, xmm1
    movdqu [rdi], xmm0
    ret

align 64
checksum_avx512:
    kxord k1, k1, k1
    mov eax, 0xFFFF
    kmovw k1, eax
    vmovdqu32 zmm0 {k1}{z}, zword [rdi]
    vpaddd zmm0 {k1}, zmm0, dword [rel zmm_seed] {1to16}
    vaddps zmm1 {k1}{rn-sae}, zmm0, zword [rel zmm_seed]
    vcompressps zword [rdi] {k1}, zmm1
    vzeroupper
    ret

align 16
fpu_reference:
    fld qword [rdi]
    fld1
    faddp st1, st0
    fsqrt
    fstp qword [rdi]
    fnstcw word [rel scratch_words]
    ret

probe_features:
    push rbx
    xor eax, eax
    cpuid
    mov eax, 1
    cpuid
    bt ecx, 27
    jnc .no_xsave
    xor ecx, ecx
    xgetbv
.no_xsave:
    rdrand r8d
    rdseed r9d
    rdtscp
    pop rbx
    ret

dispatch_checksum:
    xbegin .transaction_failed
    mov rax, [rel selected_impl]
    test rax, rax
    jz .abort
    xend
    jmp rax
.abort:
    xabort 0x7F
.transaction_failed:
    lea rax, [rel checksum_scalar]
    mov [rel selected_impl], rax     # selected after a safe fallback
    jmp rax

; A tiny MMX compatibility path remains for old plugin buffers.
checksum_mmx:
    movq mm0, [rdi]
    paddw mm0, [rdi + 8]
    psrlq mm0, 16
    movd eax, mm0
    emms
    ret
