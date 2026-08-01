#!/usr/bin/osascript
-- Compact AppleScript coverage: café λ 東京 🚀 𝌆.
property greeting : "Hello"
global runCount, lastResult

script Reporter
    on formatMessage(labelText, countValue)
        local details, messageText
        set details to {title:labelText, count:countValue, |東京 key|:"🚀 𝌆"}
        (* An outer comment
           with a nested (* café λ *) comment. *)
        set messageText to "first line: café λ ¬
second line: 東京 🚀 𝌆"
        if countValue > 0 then
            return messageText & space & (countValue as text)
        else
            error "count must be positive" number 42
        end if
    end formatMessage
end script

tell application "Finder"
    set lastResult to name of startup disk
end tell
Reporter's formatMessage(greeting, 3)
