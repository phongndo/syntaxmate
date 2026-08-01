<#
PowerShell basic fixture: café, 東京, λ, 🚀, and astral 𝌆.
#>
[CmdletBinding()]
param(
    [ValidateRange(1, 10)]
    [int] $Count = 3
)

function Get-LaunchMessage {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string] $Name)
    $meta = @{
        Label = "café 東京 λ"
        Symbol = '🚀 𝌆'
    }
    $items = 1..$Count | ForEach-Object { "$Name-$($_)" }
    if ($items.Count -gt 0) {
        return "{0} {1}: {2}" -f $meta.Symbol, $meta.Label, ($items -join ', ')
    } else {
        return 'empty'
    }
}

try {
    Get-LaunchMessage -Name 'mark'
} catch [System.Exception] {
    Write-Warning $_.Exception.Message
}
