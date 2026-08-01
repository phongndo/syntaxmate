#!/usr/bin/osascript
# AppleScript TextMate stress fixture.
-- Unicode payload: café λ 東京 🚀 𝌆.
(* Outer multiline comment begins here.
   Keywords such as tell, repeat, and "quoted text" remain comments.
   (* Nested level one: café λ.
      (* Nested level two: 東京 🚀 𝌆. *)
      Back at nested level one. *)
   Outer comment closes cleanly. *)

property fixtureName : "Grammar Mission"
prop enabled : true
property absentValue : missing value
global sharedCount, sharedText, sharedRecord
set sharedCount to 0
set sharedText to "café λ 東京 🚀 𝌆"
set sharedRecord to {title:"Launch", active:yes, retries:3, |東京 key|:"東京"}

script MissionReporter
    property prefix : "REPORT"
    on render(labelText, itemCount)
        local summaryText, metrics, escapedText
        set metrics to {count:itemCount, ratio:3.14159, exponent:6.02e+23}
        set escapedText to "quote: \" slash: \\"
        set summaryText to "first physical line café λ ¬
second physical line 東京 🚀 ¬
third physical line 𝌆"
        if itemCount is greater than 0 then
            return prefix & ": " & labelText & space & summaryText
        else
            return "empty"
        end if
    end render

    on heartbeat
        beep 1
        return current date
    end heartbeat
end script

on calculate(leftValue, rightValue)
    local totalValue, quotientValue, comparisonValue
    set totalValue to (leftValue + rightValue) * 2 - 1
    set quotientValue to totalValue div 3 mod 5
    set comparisonValue to totalValue ≥ rightValue and leftValue ≠ 0
    if comparisonValue then
        return totalValue / 2
    else
        return pi
    end if
end calculate

on place payload into destination at positionIndex
    set destination to destination & {payload}
    return item positionIndex of destination
end place

on noArguments
    return AppleScript's version
end noArguments

considering case, diacriticals and numeric strings
    set comparisonOne to "café" is equal to "CAFÉ"
    set comparisonTwo to "東京" comes before "🚀"
end considering

ignoring punctuation, hyphens and white space
    set comparisonThree to "a-b" contains "ab"
end ignoring

set numericExamples to {-42, 0, 17, 1.25, 9., 6e+8}
set booleanExamples to {true, false, yes, no, null, missing value}
set textConstants to {space, tab, return, linefeed, quote}
set calendarConstants to {January, Feb, Monday, Tue}
set styleConstants to {bold, italic, underline, all caps, small caps}
set unitExamples to {3 kilometers, 4 square feet, 5 gallons, 20 degrees Celsius}
set timeExamples to {2 seconds, 3 minutes, 4 hours, 5 days}
set classExamples to {integer, real, text, Unicode text, POSIX file, RGB color}
set referenceExamples to {first item, last word, every paragraph, item 2 thru 4}
set identifierExamples to {|name with spaces|, |café λ 東京 🚀 𝌆|}
set nestedStructures to {{1, 2, 3}, {name:"Ada", flags:{true, false}}}
set unicodeData to «data utxt00630061006600E9» as Unicode text
set rawData to «class café λ
東京 🚀 𝌆»
set illegalDataExample to «mystery»

repeat with indexValue from 1 to 3 by 1
    set sharedCount to sharedCount + indexValue
end repeat

repeat with glyphValue in {"café", "λ", "東京", "🚀", "𝌆"}
    log glyphValue
end repeat

repeat while sharedCount < 12
    set sharedCount to sharedCount + 1
end repeat

repeat until sharedCount is greater than or equal to 14
    set sharedCount to sharedCount + 1
end repeat

repeat 2 times
    delay 0.01
end repeat

repeat
    set sharedCount to sharedCount + 1
    exit repeat
end repeat

if enabled and sharedCount ≥ 0 then
    set branchName to "positive"
else if sharedCount = -1 then
    set branchName to "negative one"
else
    set branchName to "fallback"
end if

try
    set chosenPath to POSIX path of (path to desktop)
    if chosenPath is missing value then
        error "No desktop" number 404
    end if
on error errorMessage number errorNumber partial resultList from badObject to expectedType
    set sharedText to errorMessage & space & (errorNumber as text)
end try

with timeout of 30 seconds
    display dialog "Continue café λ 東京 🚀 𝌆?" buttons {"Cancel", "OK"}
end timeout

with transaction of current application
    set transactionResult to random number from 1 to 10
end transaction

using terms from application "Finder"
    set termsResult to count every item
end using terms from

tell application "Finder"
    activate
    set finderName to name
    set finderVersion to version
    set finderSelection to selection
    set homeFolder to home
    set isVisible to visible of first Finder window
    count every file of desktop
    make new folder at desktop with properties {name:"café λ 東京 🚀 𝌆"}
    reveal startup disk
    update desktop
end tell

tell application "System Events"
    set frontProcess to first application process whose frontmost is true
    tell frontProcess
        set windowCount to count every window
        click button 1 of window 1
        keystroke "λ"
        key code 36
    end tell
    set diskNames to name of every disk
    set loginCount to count every login item
    sleep
end tell

tell application "iTunes"
    set playerSnapshot to {player state, player position, sound volume}
    play first track of current playlist
    pause
    next track
    set currentTrackName to name of current track
end tell

tell application "TextMate"
    activate
    reload bundles
    insert "café λ 東京 🚀 𝌆"
    get url "txmt://open?line=1"
    set settingsCopy to print settings
end tell

tell application "Preview"
    open document 1
    set genericVersion to version
    close window 1
end tell

tell application process "Dock"
    set frontmost to true
    perform action "AXShowMenu" of UI element 1
end tell

tell sharedRecord
    set recordContents to contents
end tell

tell application "Finder" to get name of startup disk
tell current application to say "finished café λ 東京 🚀 𝌆"

set clipboardBefore to the clipboard
set the clipboard to sharedText
set clipboardMetadata to clipboard info
set folderListing to list folder (path to home folder)
set diskListing to list disks
set fileChoice to choose file with prompt "Choose a file"
set folderChoice to choose folder
set colorChoice to choose color
set menuChoice to choose from list {"café", "λ", "東京", "🚀", "𝌆"}
display alert "Fixture complete" message sharedText
say "AppleScript grammar stress complete"

set shellResult to do shell script "printf '%s' fixture"
set machineInfo to system info
set environmentHome to system attribute "HOME"
set roundedValue to round 3.75 rounding down
set randomValue to random number from 10 to 20
set utcValue to time to GMT
set resourcePath to path to resource "fixture.txt" in bundle current application
set localizedValue to localized string "FixtureTitle"
set summaryValue to summarize sharedText in 12
set offsetValue to offset of "λ" in sharedText
set asciiValue to ASCII number "A"

set ioTarget to open for access fileChoice with write permission
try
    set eofBefore to get eof ioTarget
    write sharedText to ioTarget starting at eof
    set eof ioTarget to eofBefore
    read ioTarget from 1 to eofBefore
on error ioMessage number ioNumber
    log ioMessage
end try
close access ioTarget

set loadedScript to load script fileChoice
run script loadedScript
store script loadedScript in fileChoice replacing yes
set componentList to scripting components

set operatorWords to {sharedText begins with "café", sharedText ends with "𝌆"}
set referenceWords to words 1 through 3 of sharedText
set reverseItems to reverse of nestedStructures
set restItems to rest of numericExamples
set quotedShellText to quoted form of sharedText
set characterCount to count characters of sharedText
set resultText to MissionReporter's render("final", characterCount)
return resultText
