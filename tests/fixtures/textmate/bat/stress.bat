@echo off
setlocal EnableExtensions EnableDelayedExpansion
rem Batch grammar stress fixture; safe setup followed by unreachable examples.
rem Unicode BMP and astral coverage: café λ 東京 🚀 𝌆
set "fixture_name=syntax review"
set "unicode=café λ 東京 🚀 𝌆"
set /a seed=0x2A
goto :fixture_end

rem ------------------------------------------------------------------
rem Variable forms and substitutions (unreachable when the file is run).
rem ------------------------------------------------------------------
:variables
set "plain=alpha beta gamma"
set "empty="
set "path_like=C:\fixture\input.txt"
set "percent_value=%plain%"
set "delayed_value=!plain!"
set "head=%plain:~0,5%"
set "tail=%plain:~-5%"
set "middle=%plain:~6,4%"
set "replaced=%plain:beta=BETA%"
set "removed=%plain: gamma=%"
set "fallback=%missing:=%"
set "nested=!percent_value:alpha=ALPHA!"
set "drive=%~d0"
set "script_dir=%~dp0"
set "script_name=%~nx0"
set "first_argument=%~1"
set "all_arguments=%*"
shift
echo after shift first=%1
goto :arithmetic

rem ------------------------------------------------------------------
rem SET /A operators, numbers, grouping, and delayed expansion.
rem ------------------------------------------------------------------
:arithmetic
set /a decimal=42
set /a hexadecimal=0x2A
set /a sum=decimal+hexadecimal
set /a difference=sum-5
set /a product=difference*3
set /a quotient=product/2
set /a remainder=quotient%%7
set /a shifted=(remainder<<2)
set /a shifted>>=1
set /a mask=shifted^3
set /a mask^=1
set /a bits=mask^|8
set /a bits^&=15
set /a count+=1
set /a count-=1
set /a count*=2
set /a count/=2
set /a count%%=5
set /a inverted=~bits
set /a logical=!count
set /a sequence=(count+=1, sum+=count, sum)
echo arithmetic=!sum! !remainder! !bits! !sequence!
goto :conditions

rem ------------------------------------------------------------------
rem Condition keywords, comparisons, grouping, and command chaining.
rem ------------------------------------------------------------------
:conditions
if defined plain echo plain is defined
if not defined missing echo missing is not defined
if exist "%~f0" echo this fixture exists
if not exist "Z:\definitely-missing.fixture" echo absent as expected
if errorlevel 1 echo prior status was nonzero
if cmdextversion 2 echo command extensions version checked
if "!plain!"=="alpha beta gamma" echo string equality
if /I "Alpha"=="alpha" echo case-insensitive equality
if 10 EQU 10 echo equal
if 10 NEQ 11 echo not equal
if 2 LSS 3 echo less
if 3 LEQ 3 echo less or equal
if 4 GTR 3 echo greater
if 4 GEQ 4 echo greater or equal
if defined plain (
  echo grouped true branch
  echo delayed=!plain!
) else (
  echo grouped false branch
)
verify other 2>NUL
if errorlevel 1 (
  echo captured an illustrative status
) else echo status remained zero
echo left ^& echo escaped ampersand
echo first && echo conditional success
echo first || echo conditional fallback
goto :loops

rem ------------------------------------------------------------------
rem FOR parameters, modifiers, nested loops, and multiline groups.
rem ------------------------------------------------------------------
:loops
for %%A in (alpha beta "two words" café 東京) do echo value=%%~A
for /L %%N in (1,1,5) do (
  set /a square=%%N*%%N
  echo n=%%N square=!square!
)
for /L %%N in (5,-1,1) do echo countdown=%%N
for /D %%D in ("%~dp0*") do echo directory=%%~fD
for /R "%~dp0" %%F in (*.fixture) do echo candidate=%%~nxF
for /F "tokens=1,2 delims==" %%K in ("key=value") do (
  set "key=%%K"
  set "value=%%L"
  echo !key!=!value!
)
for /F "usebackq delims=" %%L in ("%~f0") do echo source-line=%%L
for %%P in (one two) do for %%Q in (red blue) do (
  echo pair=%%P/%%Q
)
for %%F in ("C:\fixture\sample.txt") do (
  echo drive=%%~dF
  echo path=%%~pF
  echo name=%%~nF
  echo extension=%%~xF
  echo full=%%~fF
  echo short=%%~sF
  echo attributes=%%~aF
  echo time=%%~tF
  echo size=%%~zF
)
goto :redirection

rem ------------------------------------------------------------------
rem Redirection, pipes, devices, and escaped metacharacters.
rem ------------------------------------------------------------------
:redirection
echo discarded output>NUL
echo discarded error 2>NUL
echo both streams>NUL 2>&1
echo error to output 2>&1
echo output to error 1>&2
type NUL <NUL >NUL
ver >NUL
(echo grouped line one&echo grouped line two)>NUL
echo alpha|findstr /I "alpha" >NUL
echo beta ^| literal pipe
echo one ^& two ^&^& three ^|^| four
echo caret=^^ bang=^^! percent=%%
echo left parenthesis ^( and right parenthesis ^)
echo less-than ^< greater-than ^> at-sign @ colon :
echo quoted "text with spaces and !unicode!"
echo line continued with caret ^
and this physical line completes the command
goto :comments

rem ------------------------------------------------------------------
rem Supported comment spellings and label shapes.
rem ------------------------------------------------------------------
:comments
rem A normal comment with > and | remains comment text.
REM Uppercase comments are case insensitive.
rem. Dot-prefixed REM form is recognized by the grammar.
:: Colon comment: café λ 東京 🚀 𝌆
:: Another colon comment with + , ; = punctuation.
& :: Comment following a command separator.
goto :subroutines

rem ------------------------------------------------------------------
rem Labels, CALL, GOTO, parameters, and scoped delayed expansion.
rem ------------------------------------------------------------------
:subroutines
call :format "alpha beta" 17
call :delayed "value with spaces"
call :label.with-dots
goto :after_subroutines

:format
setlocal DisableDelayedExpansion
set "format_text=%~1"
set "format_number=%~2"
echo text=%format_text% number=%format_number%
endlocal & exit /b 0

:delayed
setlocal EnableDelayedExpansion
set "local_value=%~1"
set "local_value=!local_value:spaces=SPACES!"
echo delayed=!local_value:~0,10!
endlocal & exit /b 0

:label.with-dots
echo dotted label reached
exit /b 0

:after_subroutines
call echo doubled percent=%% unicode=!unicode!
goto :nested_groups

rem ------------------------------------------------------------------
rem Deeply nested groups exercise multiline lexical state restoration.
rem ------------------------------------------------------------------
:nested_groups
set /a outer=0
for %%O in (A B C) do (
  set /a outer+=1
  if !outer! GEQ 2 (
    for %%I in (1 2) do (
      set /a combined=outer*10+%%I
      if !combined! NEQ 0 (
        echo outer=%%O inner=%%I combined=!combined!
      ) else (
        echo unreachable zero
      )
    )
  ) else (
    echo first outer iteration
  )
)
goto :special_commands

rem ------------------------------------------------------------------
rem Harmless command vocabulary and control statements.
rem ------------------------------------------------------------------
:special_commands
echo on
echo off
echo.
cd .
chdir .
pushd .
popd
path
prompt $P$G
title Batch Syntax Fixture
ver
where cmd >NUL 2>&1
whoami >NUL 2>&1
call :format "final" 99
goto :fixture_end

:unused_exit
exit /b 7

:fixture_end
rem Every quote, variable, group, and label-driven path is closed at EOF.
echo fixture=%fixture_name% unicode=%unicode% seed=%seed% >NUL
endlocal
exit /b 0
