; LLVM IR stress module — café, λ, 東京, and rocket 🚀 stay safely in comments.
; RUN: opt -S -passes='default<O2>' %s | FileCheck %s
; REQUIRES: x86-registered-target
; ALLOW_RETRIES: 1
; XFAIL: windows
; CHECK-LABEL: define i32 @classify
; CHECK: switch i32
; CHECK-NOT: call void @abort
; PR4242

source_filename = "telemetry_pipeline.cc"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%struct.Record = type { i32, double, [4 x i8] }
%struct.Pair = type <{ i16, i16 }>
%struct.Handle = type opaque
$worker_group = comdat any
@program_name = private unnamed_addr constant [11 x i8] c"mark-agent\00", align 1
@unicode_bytes = private constant [14 x i8] c"caf\C3\A9 \CE\BB \F0\9F\9A\80\00", align 1
@origin = internal constant %struct.Record { i32 7, double 1.25e+1, [4 x i8] c"IR\0A\00" }, align 8
@zero_pair = common global %struct.Pair zeroinitializer, align 2
@counter = dso_local global i64 0, align 8
@tls_errno = thread_local(initialexec) global i32 0, align 4
@device_flag = addrspace(1) global i32 1, align 4
@extern_slot = external externally_initialized global ptr
@weak_hook = extern_weak global ptr
@pi = internal constant double 0x400921FB54442D18
@dispatch_table = appending global [2 x ptr] [ptr @worker, ptr @fallback], section "llvm.metadata"
@worker_alias = weak alias i32 (i32), ptr @worker

declare noalias noundef ptr @malloc(i64 noundef) #1
declare void @free(ptr nocapture) nounwind
declare i32 @puts(ptr nocapture readonly) #2
define fastcc i32 @worker(i32 noundef %work) #0 { entry: ret i32 %work }
declare coldcc i32 @fallback(i32) cold
declare i32 @might_fail(i32) uwtable
declare i32 @__gxx_personality_v0(...)
declare double @llvm.fma.f64(double, double, double) nounwind readnone speculatable
declare token @llvm.coro.id(i32, ptr, ptr, ptr)
declare void @llvm.memcpy.p0.p0.i64(
  ptr noalias nocapture writeonly,
  ptr noalias nocapture readonly,
  i64,
  i1 immarg
) argmemonly nounwind
declare void @consume_record(
  ptr byval(%struct.Record) align 8,
  i32 signext,
  ptr preallocated(%struct.Record)
) nocallback
declare void @wide_types(half, bfloat, float, fp128, x86_fp80, ppc_fp128)

define dso_local i32 @classify(i32 noundef %x, i1 %enabled) #0 !dbg !12 {
entry:
  %slot = alloca i32, align 4
  store i32 %x, ptr %slot, align 4
  #dbg_declare(ptr %slot, !17, !DIExpression(), !16)
  #dbg_value(i32 %x, !17, !DIExpression(), !16)
  %loaded = load volatile i32, ptr %slot, align 4
  %positive = icmp sgt i32 %loaded, 0
  %active = and i1 %positive, %enabled
  br i1 %active, label %calculate, label %cold.path
calculate:
  %biased = add nsw i32 %loaded, 17
  %scaled = mul nuw i32 %biased, 3
  %reduced = sub i32 %scaled, 5
  %quotient = sdiv exact i32 %reduced, 2
  %remainder = srem i32 %reduced, 11
  %left = shl i32 %quotient, 1
  %logical = lshr exact i32 %left, 1
  %signed = ashr i32 %remainder, 1
  %masked = and i32 %logical, 255
  %tagged = or i32 %masked, 256
  %mixed = xor i32 %tagged, %signed
  br label %merge
cold.path:
  %absolute = sub nsw i32 0, %loaded
  br label %merge
merge:
  %candidate = phi i32 [ %mixed, %calculate ], [ %absolute, %cold.path ]
  %stable = freeze i32 %candidate
  %choice = select i1 %enabled, i32 %stable, i32 poison
  switch i32 %choice, label %other [
    i32 0, label %zero
    i32 1, label %one
  ]
zero:
  ret i32 10
one:
  ret i32 20
other:
  %unsigned.q = udiv i32 %choice, 7
  %unsigned.r = urem i32 %choice, 7
  %answer = add i32 %unsigned.q, %unsigned.r
  ret i32 %answer
}

define i32 @update_cell(ptr nonnull align 4 %cell, i32 %value) nounwind {
entry:
  %old = load atomic i32, ptr %cell acquire, align 4
  %next = add i32 %old, %value
  store atomic i32 %next, ptr %cell release, align 4
  fence syncscope("singlethread") seq_cst
  %pair = cmpxchg weak volatile ptr %cell, i32 %next, i32 %value acq_rel acquire
  %observed = extractvalue { i32, i1 } %pair, 0
  %won = extractvalue { i32, i1 } %pair, 1
  %prior = atomicrmw xchg ptr %cell, i32 %observed monotonic, align 4
  %total = atomicrmw add ptr @counter, i64 1 seq_cst, align 8
  %ok = select i1 %won, i32 %prior, i32 %old
  ret i32 %ok
}

define ptr @copy_record(ptr nocapture readonly %source) #3 {
entry:
  %bytes = call noalias ptr @malloc(i64 24)
  %isnull = icmp eq ptr %bytes, null
  br i1 %isnull, label %failed, label %copy
copy:
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %bytes, ptr align 8 %source, i64 24, i1 false)
  %field = getelementptr inbounds %struct.Record, ptr %bytes, i32 0, i32 0
  store i32 99, ptr %field, align 8
  ret ptr %bytes
failed:
  ret ptr null
}

define double @convert_and_scale(i16 %small, i64 %bits, ptr %address) strictfp {
entry:
  %unsigned = zext i16 %small to i32
  %signed = sext i16 %small to i64
  %narrow = trunc i64 %signed to i8
  %as.float = uitofp i32 %unsigned to float
  %as.double = sitofp i64 %signed to double
  %extended = fpext float %as.float to double
  %sum = fadd double %as.double, %extended
  %short = fptrunc double %sum to float
  %back.signed = fptosi float %short to i32
  %back.unsigned = fptoui float %short to i32
  %address.bits = ptrtoint ptr %address to i64
  %roundtrip = inttoptr i64 %address.bits to ptr
  %remote = addrspacecast ptr %roundtrip to ptr addrspace(1)
  %decoded = bitcast i64 %bits to double
  %noise = add i32 %back.signed, %back.unsigned
  ret double %decoded
}

define { <4 x i32>, %struct.Pair } @pack_vectors(<4 x i32> %values, i32 %lane) {
entry:
  %element = extractelement <4 x i32> %values, i32 %lane
  %inserted = insertelement <4 x i32> %values, i32 42, i32 2
  %reversed = shufflevector <4 x i32> %inserted, <4 x i32> undef,
                            <4 x i32> <i32 3, i32 2, i32 1, i32 0>
  %vectors = insertvalue { <4 x i32>, %struct.Pair } undef, <4 x i32> %reversed, 0
  %low = trunc i32 %element to i16
  %with.low = insertvalue { <4 x i32>, %struct.Pair } %vectors, i16 %low, 1, 0
  %complete = insertvalue { <4 x i32>, %struct.Pair } %with.low, i16 -1, 1, 1
  %check = extractvalue { <4 x i32>, %struct.Pair } %complete, 0
  ret { <4 x i32>, %struct.Pair } %complete
}

define double @polynomial(double %x, double %y) #2 {
entry:
  %negated = fneg fast double %x
  %product = fmul nnan ninf double %negated, %y
  %sum = fadd reassoc nsz double %product, 1.0e+0
  %difference = fsub double %sum, %x
  %ratio = fdiv arcp double %difference, %y
  %modulus = frem double %ratio, 3.0e+0
  %ordered = fcmp oge double %modulus, 0.0
  %unordered = fcmp uno double %x, %y
  %finite = xor i1 %ordered, %unordered
  %fused = call double @llvm.fma.f64(double %x, double %y, double %modulus)
  %result = select i1 %finite, double %fused, double 0x7FF8000000000000
  ret double %result
}

define i32 @invoke_worker(i32 %input) personality ptr @__gxx_personality_v0 {
entry:
  %result = invoke i32 @might_fail(i32 %input)
            to label %returned unwind label %exception
returned:
  %tail = tail call fastcc i32 @worker(i32 %result)
  ret i32 %tail
exception:
  %landing = landingpad { ptr, i32 }
             cleanup
             catch ptr null
  %reason = extractvalue { ptr, i32 } %landing, 1
  resume { ptr, i32 } %landing
}

define void @computed_dispatch(i1 %take.left) {
entry:
  %destination = select i1 %take.left, ptr blockaddress(@computed_dispatch, %left),
                                      ptr blockaddress(@computed_dispatch, %right)
  indirectbr ptr %destination, [label %left, label %right]
left:
  call void asm sideeffect inteldialect "pause", "~{dirflag},~{fpsr},~{flags}"()
  br label %done
right:
  br label %done
done:
  ret void
}

define void @funclet_paths() personality ptr @__gxx_personality_v0 {
entry:
  %ignored = invoke i32 @might_fail(i32 0) to label %done unwind label %dispatch
dispatch:
  %switch = catchswitch within none [label %handler] unwind label %cleanup
handler:
  %catch = catchpad within %switch [ptr null, i32 64, ptr null]
  catchret from %catch to label %done
cleanup:
  %pad = cleanuppad within none []
  cleanupret from %pad unwind to caller
done:
  ret void
}

define i32 @read_variadic(ptr %arguments) {
entry:
  %value = va_arg ptr %arguments, i32
  #dbg_label(!18, !16)
  #dbg_assign(i32 %value, !17, !DIExpression(), !20, ptr %arguments, !DIExpression(), !16)
  ret i32 %value
}

define void @trap_if_null(ptr %candidate) cold noreturn {
entry:
  %missing = icmp eq ptr %candidate, null
  br i1 %missing, label %trap, label %impossible
trap:
  unreachable
impossible:
  unreachable
}

attributes #0 = { mustprogress nounwind uwtable "frame-pointer"="all" }
attributes #1 = { allocsize(0) nofree nounwind willreturn }
attributes #2 = { nocallback nofree nosync nounwind readonly }
attributes #3 = { noinline optnone sanitize_address }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.dbg.cu = !{!10}
!0 = !{i32 2, !"Dwarf Version", i32 5}
!1 = !{i32 2, !"Debug Info Version", i32 3}
!2 = !{!"mark syntax stress: café λ 東京 🚀"}
!10 = distinct !DICompileUnit(language: DW_LANG_C_plus_plus, file: !11, producer: "mark", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug)
!11 = !DIFile(filename: "telemetry_pipeline.cc", directory: "/tmp/mark")
!12 = distinct !DISubprogram(name: "classify", scope: !11, file: !11, line: 42, type: !13, scopeLine: 42, spFlags: DISPFlagDefinition, unit: !10)
!13 = !DISubroutineType(types: !14)
!14 = !{!15}
!15 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!16 = !DILocation(line: 44, column: 3, scope: !12)
!17 = !DILocalVariable(name: "value", scope: !12, file: !11, line: 43, type: !15)
!18 = !DILabel(scope: !12, name: "variadic", file: !11, line: 90)
!20 = distinct !DIAssignID()
