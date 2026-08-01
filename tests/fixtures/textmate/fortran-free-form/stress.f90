#define DEFAULT_COUNT 4
#define CLAMPED(x,lo,hi) \
  max((lo), min((hi), (x)))
#if defined(ENABLE_TRACE) && DEFAULT_COUNT > 0
#define TRACE_MESSAGE "trace enabled"
#else
#define TRACE_MESSAGE "trace disabled"
#endif
module fixture_math
  use, intrinsic :: iso_fortran_env, only: error_unit, int32, real64
  use, intrinsic :: iso_c_binding, only: c_int
  implicit none
  private
  public :: sample_t, apply_transform, clamp_value
  public :: inspect_any, describe_rank, tolerance
  real(real64), parameter :: tolerance = 1.0e-10_real64
  enum, bind(c)
    enumerator :: state_idle = 0, state_ready = 1
    enumerator :: state_done = 2
  end enum
  abstract interface
    pure function transform_fn(x) result(y)
      import :: real64
      real(real64), intent(in) :: x
      real(real64) :: y
    end function transform_fn
  end interface
  interface clamp_value
    module procedure clamp_real
  end interface clamp_value
  type, public :: sample_t
    integer(int32) :: id = 0_int32
    character(len=:), allocatable :: label
    real(real64), allocatable :: values(:)
  contains
    procedure, public :: mean => sample_mean
    procedure, public :: scale => sample_scale
    procedure, pass(lhs), private :: add_samples
    generic, public :: operator(+) => add_samples
    final :: finalize_sample
  end type sample_t
contains
  pure function sample_mean(self) result(answer)
    class(sample_t), intent(in) :: self
    real(real64) :: answer
    if (allocated(self%values) .and. size(self%values) > 0) then
      answer = sum(self%values) / real(size(self%values), real64)
    else
      answer = 0.0_real64
    end if
  end function sample_mean
  subroutine sample_scale(self, factor)
    class(sample_t), intent(inout) :: self
    real(real64), intent(in), value :: factor
    if (.not. allocated(self%values)) return
    where (self%values >= 0.0_real64)
      self%values = self%values * factor
    elsewhere
      self%values = -abs(self%values) * factor
    end where
  end subroutine sample_scale
  function add_samples(lhs, rhs) result(combined)
    class(sample_t), intent(in) :: lhs
    type(sample_t), intent(in) :: rhs
    type(sample_t) :: combined
    integer :: count
    count = min(size(lhs%values), size(rhs%values))
    combined%id = max(lhs%id, rhs%id)
    combined%label = lhs%label // "+" // rhs%label
    allocate(combined%values(count))
    combined%values = lhs%values(:count) + rhs%values(:count)
  end function add_samples
  subroutine finalize_sample(self)
    type(sample_t), intent(inout) :: self
    if (allocated(self%values)) deallocate(self%values)
    if (allocated(self%label)) deallocate(self%label)
  end subroutine finalize_sample
  subroutine apply_transform(self, operation)
    type(sample_t), intent(inout) :: self
    procedure(transform_fn) :: operation
    integer :: i
    do concurrent (i = 1:size(self%values))
      self%values(i) = operation(self%values(i))
    end do
  end subroutine apply_transform
  elemental real(real64) function clamp_real(value, lower, upper)
    real(real64), intent(in) :: value, lower, upper
    clamp_real = max(lower, min(upper, value))
  end function clamp_real
  subroutine inspect_any(item)
    class(*), intent(in) :: item
    select type (item)
    type is (integer)
      write (*, '(A,I0)') "integer=", item
    type is (sample_t)
      write (*, '(A,A)') "sample=", item%label
    class default
      write (error_unit, '(A)') "unknown dynamic type"
    end select
  end subroutine inspect_any
  subroutine describe_rank(data)
    real(real64), intent(in) :: data(..)
    select rank (view => data)
    rank (0)
      write (*, '(A,F7.3)') "scalar ", view
    rank (1)
      write (*, '(A,I0)') "vector size ", size(view)
    rank default
      write (*, '(A,I0)') "other rank ", rank(view)
    end select
  end subroutine describe_rank

end module fixture_math

program stress_free_form
  use, intrinsic :: iso_fortran_env, only: int32, real64
  use fixture_math, only: sample_t, apply_transform, clamp_value, &
    & inspect_any, describe_rank, tolerance
  implicit none

  character(len=*), parameter :: unicode_text = "BMP λ, 雪 and astral 🚀🛰; &
    &continued with ""quotes"" and an apostrophe's."
  character(len=*), parameter :: single_text = 'naïve Ω says ''hello'' to 😀'
  type(sample_t) :: left, right, combined
  real(real64), allocatable :: scratch(:)
  real(real64), target :: target_value
  real(real64), pointer :: pointer_value => null()
  integer :: i, unit, status, tick
  integer(int32) :: phase
  character(len=48) :: record
  logical :: exists

  left = sample_t(id=1_int32, label="left", &
    & values=[1.0_real64, -2.0_real64, 3.5_real64, 5.0_real64])
  right = sample_t(id=2_int32, label="right", &
    & values=(/ 0.5_real64, 2.0_real64, 4.5_real64, 8.0_real64 /))
  combined = left + right
  phase = 1_int32

  target_value = 2.0_real64
  pointer_value => target_value
  pointer_value = pointer_value ** 3
  allocate(scratch(DEFAULT_COUNT), source=0.0_real64, stat=status)
  if (status /= 0) error stop "allocation failed"

  call random_number(scratch)
  scratch = clamp_value(scratch, 0.2_real64, 0.8_real64)
  call move_alloc(scratch, left%values)
  call left%scale(factor=2.0_real64)
  call apply_transform(left, square)

  associate (average => left%mean(), total => sum(left%values))
    write (*, '(A,F8.3,1X,A,F8.3)') "mean", average, "sum", total
  end associate

  block
    real(real64) :: normalized(size(combined%values))

    normalized = combined%values / max(1.0_real64, maxval(abs(combined%values)))
    combined%values = merge(normalized, 0.0_real64, &
      & abs(normalized) > tolerance)
  end block

  outer: do i = 1, size(combined%values)
    if (combined%values(i) < 0.0_real64) cycle outer
    select case (i)
    case (1)
      combined%values(i) = combined%values(i) + 1.0_real64
    case (2:3)
      combined%values(i) = combined%values(i) / 2.0_real64
    case default
      exit outer
    end select
  end do outer

  where (right%values > 1.0_real64)
    right%values = right%values - 1.0_real64
  elsewhere (right%values == 1.0_real64)
    right%values = 0.0_real64
  elsewhere
    right%values = right%values + 1.0_real64
  end where

  forall (i = 1:size(right%values), right%values(i) >= 0.0_real64)
    right%values(i) = sqrt(right%values(i))
  end forall

  if (associated(pointer_value, target_value)) then
    phase = phase + 1_int32
  else if (pointer_value == 0.0_real64) then
    phase = -1_int32
  else
    stop "pointer state is inconsistent"
  end if

  call inspect_any(phase)
  call inspect_any(combined)
  call describe_rank(target_value)
  call describe_rank(right%values)

  inquire (file="fixture.tmp", exist=exists)
  open (newunit=unit, status="scratch", action="readwrite", &
    & form="formatted", iostat=status)
  if (status == 0) then
    write (unit, '(A,1X,F8.3)') trim(unicode_text), combined%mean()
    write (unit, 100) phase, single_text
    rewind (unit)
    read (unit, '(A)', iostat=status) record
    flush (unit)
    close (unit)
  end if
100 format ('phase=', I0, 1X, A)

  call system_clock(count=tick)
  critical
    phase = phase + int(mod(tick, 2), int32)
  end critical
  sync all

  write (*, '(A)') trim(record)
  write (*, '(A,L1,1X,A)') "existing file: ", exists, single_text
  if (phase < 0_int32) error stop 7

  nullify(pointer_value)
  if (allocated(scratch)) deallocate(scratch)

contains

  pure function square(value) result(result_value)
    real(real64), intent(in) :: value
    real(real64) :: result_value

    result_value = value * value
  end function square

end program stress_free_form
