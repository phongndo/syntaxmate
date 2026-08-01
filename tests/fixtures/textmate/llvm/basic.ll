; LLVM IR: café, π, and an astral rocket 🚀
; RUN: opt -S %s
source_filename = "orbit.ll"
target triple = "x86_64-unknown-linux-gnu"
@banner = private unnamed_addr constant [4 x i8] c"hi\0A\00", align 1
@ratio = internal constant double 1.25e+1
declare i32 @puts(ptr nocapture readonly)
define i32 @choose(i32 %x, i1 %ready) nounwind {
entry:
  %slot = alloca i32, align 4
  store i32 %x, ptr %slot, align 4
  %loaded = load i32, ptr %slot, align 4
  %positive = icmp sgt i32 %loaded, 0
  %take = and i1 %ready, %positive
  br i1 %take, label %then, label %otherwise
then:
  %sum = add nsw i32 %loaded, 42
  br label %merge
otherwise:
  %neg = sub i32 0, %loaded
  br label %merge
merge:
  %result = phi i32 [ %sum, %then ], [ %neg, %otherwise ]
  %small = trunc i32 %result to i8
  %wide = zext i8 %small to i32
  %answer = select i1 true, i32 %wide, i32 poison
  ret i32 %answer
}
!0 = !{!"debug tag", i32 7}
