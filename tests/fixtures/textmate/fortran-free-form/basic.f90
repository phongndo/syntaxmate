program basic_free_form
  use, intrinsic :: iso_fortran_env, only: real64
  implicit none
  character(len=*), parameter :: greeting = "Hello, λ雪 and 🚀; &
    &continued safely across a source line."
  integer :: i
  real(real64) :: values(4)
  logical :: rising

  values = [1.0_real64, 2.5_real64, &
    & 4.0_real64, 8.0_real64]
  rising = .true.
  write (*, '(A)') greeting

  do i = 1, size(values)
    if (rising .and. values(i) >= 2.0_real64) then
      write (*, '(I2,2X,F6.2)') i, sqrt(values(i))
    else if (values(i) < 0.0_real64) then
      error stop "unexpected negative value"
    else
      cycle
    end if
  end do
end program basic_free_form
