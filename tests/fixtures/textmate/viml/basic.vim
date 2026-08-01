" vim: set ft=vim sw=2 et:
" Compact Vimscript fixture: café λ 東京 🚀 𝌆.
set nocompatible hidden number
let g:greeting = "Hello, café λ 東京 🚀 𝌆"
let s:matcher = /^(hello|world)\s\+vim$/

function! s:Welcome(name) abort
  let l:message = printf('%s, %s', g:greeting, a:name)
  if empty(a:name) || a:name ==# 'anonymous'
    return 'nobody'
  elseif a:name =~# s:matcher
    return l:message
  else
    return l:message . '!'
  endif
endfunction

command! -bang -nargs=1 -complete=file Greet call s:Welcome(<args>)
nnoremap <silent> <leader>g :Greet café🚀<CR>
augroup fixture_messages
  autocmd!
  autocmd BufReadPost *.vim echo "loaded λ 𝌆"
augroup END
syntax keyword FixtureTodo TODO contained
highlight default link FixtureTodo Todo
