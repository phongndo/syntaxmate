@echo off
setlocal EnableExtensions EnableDelayedExpansion
rem Basic batch fixture: café λ 東京 🚀 𝌆
set "name=café λ 東京 🚀 𝌆"
set /a total=1+2*3
set "slice=%name:~0,4%"
for %%G in (alpha beta gamma) do (
  set /a total+=1
  echo item=%%G total=!total! name=!name!
)
if defined name if !total! GEQ 10 (
  echo comparison passed ^& metacharacters ^< ^> ^| are escaped
) else (
  echo comparison failed
)
call :show "%slice%" >NUL 2>&1
goto :done

:show
echo argument=%~1 script=%~nx0
exit /b 0

:done
endlocal
