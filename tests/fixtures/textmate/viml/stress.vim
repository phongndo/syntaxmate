" vim: set ft=vim sw=2 et foldmethod=marker:
" A realistic navigation plugin fixture with café, λ, 東京, 🚀, and 𝌆.
" Repository coverage: commands, mappings, syntax, options, and expressions. {{{1
if exists('g:loaded_orbit_fixture')
  finish
endif
let g:loaded_orbit_fixture = true

set nocompatible
set hidden
set number
set relativenumber
set ignorecase
set smartcase
set incsearch
set noerrorbells
set nowrap
set tabstop=2
set shiftwidth=2
set softtabstop=2
set expandtab
set background=dark
set completeopt=menu,menuone,noselect
set wildmode=longest:full,full
let &statusline = '%f %m %= %l:%c'
let &l:commentstring = '# %s'

let s:plugin_name = 'orbit-fixture'
let s:home_url = 'https://example.test/café/orbit'
let s:welcome = "Launch 🚀 toward 東京 with λ and 𝌆"
let s:search_pattern = /\v^(TODO|FIXME):\s+(.+)$/
let s:empty_pattern = /^$/
let s:default_limit = 42
let s:enabled = true
let s:preview = false
let g:orbit_marks = []
let g:orbit_config = {'limit': 25, 'wrap': false}

""" Build a display label for one location.
" The documentation comment remains active on this quoted line 🚀.
" It closes before the function declaration below.
function! s:Label(path, line, text) abort
  let l:tail = fnamemodify(a:path, ':t')
  let l:clean = substitute(a:text, '\s\+', ' ', 'g')
  return printf('%s:%d — %s', l:tail, a:line, l:clean)
endfunction

function! s:Normalize(item) abort
  if type(a:item) == type('')
    return {'path': a:item, 'line': 1, 'text': ''}
  endif
  if !has_key(a:item, 'path')
    throw 'orbit: item has no path'
  endif
  let l:copy = copy(a:item)
  let l:copy.line = get(l:copy, 'line', 1)
  let l:copy.text = get(l:copy, 'text', 'café λ')
  return l:copy
endfunction

function! s:Add(path, line, text) abort
  let l:item = {'path': a:path, 'line': a:line, 'text': a:text}
  call add(g:orbit_marks, s:Normalize(l:item))
  return len(g:orbit_marks)
endfunction

function! s:Remove(index) abort
  if a:index < 0 || a:index >= len(g:orbit_marks)
    echohl WarningMsg
    echom 'orbit: index out of range'
    echohl None
    return false
  endif
  call remove(g:orbit_marks, a:index)
  return true
endfunction

function! s:Find(query) abort
  let l:matches = []
  for l:item in g:orbit_marks
    let l:label = s:Label(l:item.path, l:item.line, l:item.text)
    if l:label =~? a:query
      call add(l:matches, l:item)
    endif
  endfor
  return l:matches
endfunction

function! s:Render(items) abort
  let l:lines = ['Orbit marks — 東京 🚀']
  let l:index = 0
  while l:index < len(a:items)
    let l:item = a:items[l:index]
    call add(l:lines, printf('%2d %s', l:index + 1,
          \ s:Label(l:item.path, l:item.line, l:item.text)))
    let l:index += 1
  endwhile
  return join(l:lines, "\n")
endfunction

function! s:Open(index) abort
  try
    let l:item = g:orbit_marks[a:index]
    execute 'edit ' . fnameescape(l:item.path)
    execute l:item.line
    normal! zz
    redraw
    echo s:Label(l:item.path, l:item.line, l:item.text)
  catch /^Vim\%((\a\+)\)\=:E684/
    echohl ErrorMsg
    echom 'orbit: invalid mark index'
    echohl None
  catch
    echohl ErrorMsg
    echom 'orbit: ' . v:exception
    echohl None
  finally
    let v:errmsg = ''
  endtry
endfunction

function! s:CollectBuffer() abort
  let l:number = 1
  for l:text in getline(1, '$')
    if l:text =~# s:search_pattern
      call s:Add(expand('%:p'), l:number, l:text)
    endif
    let l:number = l:number + 1
  endfor
  return len(g:orbit_marks)
endfunction

function! s:Complete(arglead, cmdline, cursorpos) abort
  let l:candidates = ['add', 'clear', 'list', 'open', 'scan']
  return filter(l:candidates, 'v:val =~? "^" . a:arglead')
endfunction

function! s:Dispatch(action, bang) abort
  if a:action ==# 'add'
    call s:Add(expand('%:p'), line('.'), getline('.'))
  elseif a:action ==# 'clear'
    let g:orbit_marks = []
  elseif a:action ==# 'list'
    echo s:Render(g:orbit_marks)
  elseif a:action ==# 'open'
    call s:Open(0)
  elseif a:action ==# 'scan'
    call s:CollectBuffer()
  else
    throw 'orbit: unknown action ' . a:action
  endif
  if a:bang
    redraw!
  endif
endfunction

command! -bang -nargs=1 -complete=customlist,s:Complete Orbit
      \ call s:Dispatch(<q-args>, <bang>0)
command! -nargs=0 OrbitList echo s:Render(g:orbit_marks)
command! -nargs=1 OrbitOpen call s:Open(<args>)

nnoremap <silent> <leader>oa :Orbit add<CR>
nnoremap <silent> <leader>ol :Orbit list<CR>
nnoremap <silent> <leader>os :Orbit scan<CR>
nnoremap <buffer> <F8> :OrbitOpen 0<CR>
inoremap <expr> <C-L> pumvisible() ? '<C-N>' : '<Right>'
vnoremap <silent> <leader>oc :<C-U>Orbit clear<CR>

augroup orbit_fixture
  autocmd!
  autocmd BufReadPost * if line("'\"") > 0 | execute "normal! g`\"" | endif
  autocmd BufWritePost *.vim silent call s:CollectBuffer()
  autocmd FileType vim setlocal foldmethod=marker
  autocmd ColorScheme * highlight default link OrbitHeader Title
augroup END

syntax case match
syntax keyword OrbitTodo TODO FIXME BUG contained
syntax match OrbitIndex /^\s*\d\+/
syntax region OrbitQuoted start=+"+ skip=+\\"+ end=+"+ keepend
syntax cluster OrbitNotes contains=OrbitTodo,OrbitIndex
highlight default link OrbitTodo Todo
highlight default link OrbitIndex Number
highlight default link OrbitQuoted String
highlight default link OrbitHeader Title

colorscheme default
silent! call s:Add(expand('%:p'), 1, s:welcome)
redir => s:messages
silent messages
redir END
let s:last_status = v:statusmsg
let s:shell_failed = v:shell_error
unlet! s:messages
" Fold closes here. }}}1
