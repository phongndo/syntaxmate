# PowerShell smoke fixture: café λ
function Get-Greeting([string]$Name) {
    "hello $Name"
}
Write-Host (Get-Greeting -Name "mark")
