# PowerShell TextMate stress fixture: café, λ, 中文, 😀
# This file is parsed and tokenized, not intended as an operational script.
<#
.SYNOPSIS
    Exercises broad, valid PowerShell syntax without destructive behavior.
.DESCRIPTION
    Covers comments, declarations, strings, collections, flow, and operators.
.EXAMPLE
    Parse this fixture with a PowerShell parser; do not rely on its output.
.NOTES
    Unicode includes BMP text (café λ 中文) and an astral emoji (😀).
#>
[CmdletBinding(SupportsShouldProcess = $false)]
param(
    [Parameter(Position = 0)]
    [ValidateSet('Preview', 'Inspect', 'Unicode')]
    [string] $Mode = 'Preview',
    [Parameter(Position = 1)]
    [ValidateRange(1, 100)]
    [int] $Limit = 10
)
# Variables, scope qualifiers, automatic values, and unusual variable names.
$script:FixtureName = 'powershell-stress'
$local:CurrentMode = $Mode
$private:ScratchValue = 0
${Fixture Label} = 'café λ 中文 😀'
$homeSnapshot = $global:HOME
$pathSnapshot = $env:PATH
$null = $private:ScratchValue
# Numeric, Boolean, null, type, range, and unary literals/expressions.
$decimalInteger = 1_024
$hexInteger = 0x2A
$longInteger = 9223372036854775807L
$realNumber = 3.14159
$scientific = 6.022e23
$decimalNumber = 19.95d
$sizeLiteral = 2kb
$truthValues = $true, $false, $null
$rangeValues = -2..2
$typedCharacter = [char]0x03BB
# Quoted strings, interpolation, subexpressions, and backtick escapes.
$singleQuoted = 'Literal $Mode, C:\temp, and ''quoted'' text'
$doubleQuoted = "mode=$Mode; label=${Fixture Label}; limit=$($Limit + 1)"
$escapedText = "first line`nsecond line`t`"quoted`" and a literal `$Mode"
$unicodeText = "café λ 中文 😀"
$singleHere = @'
Single-quoted here-string: $Mode is not expanded.
Quotes ' and " remain ordinary text; Unicode: café 中文 😀.
'@
$doubleHere = @"
Double-quoted here-string for $Mode.
The computed limit is $($Limit * 2), and escaped dollar is `$Limit.
"@
# Arrays, nested arrays, typed arrays, and hashtable forms.
$numbers = @(1, 2, 3, 5, 8)
[string[]] $words = 'alpha', 'café', 'λ', '中文', '😀'
$nested = @(,('north', 'south'), ('east', 'west'))
$settings = @{ Mode = $Mode; Limit = $Limit; Enabled = $true
    'Unicode key' = ${Fixture Label} }
$orderedSettings = [ordered]@{ First = 1; Second = 2 }
enum FixtureState {
    Unknown = 0
    Ready = 1
    Complete = 2
}
class FixtureRecord {
    [string] $Name
    [int] $Count = 0
    [FixtureState] $State = [FixtureState]::Unknown

    FixtureRecord([string] $name, [int] $count) {
        $this.Name = $name
        $this.Count = $count
    }

    [string] Describe() {
        return '{0}:{1}:{2}' -f $this.Name, $this.Count, $this.State
    }

    static [FixtureRecord] Create([string] $name) {
        return [FixtureRecord]::new($name, 0)
    }
}
# A conventional function and an advanced, pipeline-aware function.
function Join-FixtureWord([string] $Left, [string] $Right = 'default') {
    return "${Left}::$Right"
}

function Get-FixtureSummary {
    [CmdletBinding(DefaultParameterSetName = 'ByName')]
    [OutputType([pscustomobject])]
    param(
        [Parameter(Mandatory, Position = 0, ValueFromPipeline,
                   ValueFromPipelineByPropertyName)]
        [Alias('Label')]
        [ValidatePattern('^\p{L}')]
        [string] $Name,

        [Parameter(Position = 1)]
        [ValidateRange(0, 1000)]
        [int] $Count = 1,

        [AllowEmptyCollection()]
        [string[]] $Tags = @()
    )

    begin { $sequence = 0 }
    process {
        $sequence += 1
        [pscustomobject]@{
            Name = $Name
            Count = $Count
            Tags = $Tags
            Sequence = $sequence
            ParameterSet = $PSCmdlet.ParameterSetName
        }
    }
    end { Write-Verbose "processed $sequence item(s)" }
}
# Constructors, properties, instance methods, and static member access.
$record = [FixtureRecord]::Create('café')
$record.Count = [Math]::Max($Limit, 1)
$record.State = [FixtureState]::Ready
$description = $record.Describe()
$list = [System.Collections.Generic.List[string]]::new()
$null = $list.Add('λ')
$dateKind = [DateTimeKind]::Utc
# Arithmetic, comparison, logical, bitwise, type, and collection operators.
$sum = 1 + 2 * 3 - 4 / 2
$comparison = ($sum -ge 5) -and ('café' -like 'caf*')
$caseSensitive = 'λ' -ceq 'Λ'
$collectionTest = (2 -in $numbers) -and ($numbers -contains 8)
$typeTest = ($record -is [FixtureRecord]) -and ($Limit -isnot [string])
$converted = 42 -as [string]
$bits = (0x0F -band 0x03) -bor 0x10
$shifted = 1 -shl 4
$negated = -not (!$false)
# Regex, replacement, split/join, and formatting operators.
if ('abc-123' -match '^(?<letters>\p{L}+)-(?<digits>\d+)$') {
    $regexGroups = $Matches.letters, $Matches.digits
}
$replacement = 'abc-123' -replace '(\p{L}+)-(\d+)', '$1:$2'
$escapedRegex = [regex]::Escape('a+b?')
$splitWords = 'red, green;blue' -split '\s*[,;]\s*'
$joinedWords = $splitWords -join ' | '
$formatted = 'name={0,-8} count={1:D3}' -f $record.Name, $record.Count
# Pipelines, $_, scriptblocks, invocation, and both splatting forms.
$predicate = { param([int] $Value) $Value -gt 2 }
$predicateResult = & $predicate 3
$pipelineResult = $numbers |
    Where-Object { $_ % 2 -eq 1 } |
    ForEach-Object { [Math]::Pow($_, 2) } |
    Sort-Object -Descending
$namedArguments = @{
    Name = 'lambda λ'
    Count = 2
    Tags = @('BMP', 'astral 😀')
}
$namedResult = Get-FixtureSummary @namedArguments
$positionalArguments = @('中文', 3)
$positionalResult = Get-FixtureSummary @positionalArguments
# Conditional branches and switch clauses with typed, regex, and case patterns.
if ($Limit -gt 50) {
    $limitClass = 'large'
} elseif ($Limit -gt 10) {
    $limitClass = 'medium'
} else {
    $limitClass = 'small'
}
switch ($record.State) {
    ([FixtureState]::Ready) { $stateText = 'ready'; break }
    default { $stateText = 'other' }
}
switch -Regex -CaseSensitive ($words) {
    '^a' { $wordClass = 'latin-a'; continue }
    '^λ$' { $wordClass = 'lambda'; continue }
    default { $wordClass = 'other' }
}
# for, foreach, while, do/while, and do/until loops remain bounded.
$loopTotal = 0
for ($index = 0; $index -lt [Math]::Min($Limit, 3); $index++) {
    $loopTotal += $index
}
foreach ($entry in $orderedSettings.GetEnumerator()) {
    $null = '{0}={1}' -f $entry.Key, $entry.Value
}
$cursor = 0
while ($cursor -lt 2) { $cursor++ }
do { $cursor-- } while ($cursor -gt 0)
do { $cursor++ } until ($cursor -ge 1)
# Exceptions, typed catches, finally, trap, and a non-invoked throwing block.
try {
    $parsedNumber = [int]::Parse('42')
    if ($Limit -lt 0) { throw [ArgumentOutOfRangeException]::new('Limit') }
} catch [System.ArgumentOutOfRangeException] {
    $errorText = $_.Exception.Message
} catch {
    $errorText = $_.ToString()
} finally {
    $finallyReached = $true
}
function Invoke-FixtureTrap([scriptblock] $Action) {
    trap [System.Exception] { $script:LastTrap = $_.Exception.Message; continue }
    & $Action
}
$throwingBlock = { throw 'fixture exception: café 😀' }
# End in the root lexical state with harmless result construction.
$summary = [pscustomobject]@{ Fixture = $script:FixtureName
    Description = $description; Unicode = $unicodeText; Values = $pipelineResult }
$summary | Select-Object Fixture, Description, Unicode, Values
