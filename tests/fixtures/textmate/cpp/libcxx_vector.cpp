// Reduced libc++-style header fixture for oracle parity.
#ifndef _LIBCPP_VECTOR
#define _LIBCPP_VECTOR

#include <__config>
#include <__memory/allocator.h>
#include <initializer_list>

_LIBCPP_BEGIN_NAMESPACE_STD

template <class _Tp, class _Allocator = allocator<_Tp> >
class _LIBCPP_TEMPLATE_VIS vector {
public:
  typedef _Tp value_type;
  typedef _Allocator allocator_type;
  typedef value_type& reference;
  typedef const value_type& const_reference;

  _LIBCPP_CONSTEXPR_SINCE_CXX20 vector() noexcept(noexcept(allocator_type())) {}

  template <class _InputIterator,
            __enable_if_t<__has_exactly_input_iterator_category<_InputIterator>::value, int> = 0>
  _LIBCPP_HIDE_FROM_ABI vector(_InputIterator __first, _InputIterator __last) {
    for (; __first != __last; ++__first)
      push_back(*__first);
  }

  _LIBCPP_HIDE_FROM_ABI void push_back(const_reference __x) {
    __annotate_contiguous_container(data(), data() + capacity(), data() + size(), data() + size() + 1);
  }

#if _LIBCPP_STD_VER >= 20
  [[nodiscard]] _LIBCPP_HIDE_FROM_ABI constexpr bool empty() const noexcept { return size() == 0; }
#endif
};

_LIBCPP_END_NAMESPACE_STD

#endif // _LIBCPP_VECTOR
