### Borealis desktop localization stress fixture 🚀
### Exercises messages, terms, attributes, selectors, functions, and Unicode.

## Brand terms

-brand-short-name = Borealis
-brand-full-name = Borealis Web Browser
-vendor-name = Aurora Labs
-sync-brand = Borealis Sync
-support-site = Aurora Support
-case-brand = Borealis
    .gender = neuter

## Window chrome and menus

main-window =
    .title = { -brand-full-name }
    .style = min-width: 48em;
private-window-title = Private Browsing — { -brand-short-name }
menu-file =
    .label = File
    .accesskey = F
menu-edit =
    .label = Edit
    .accesskey = E
menu-view =
    .label = View
    .accesskey = V
menu-history =
    .label = History
    .accesskey = s
menu-bookmarks =
    .label = Bookmarks
    .accesskey = B
menu-tools =
    .label = Tools
    .accesskey = T
menu-help =
    .label = Help
    .accesskey = H
menu-new-tab =
    .label = New Tab
    .key = T
menu-new-window =
    .label = New Window
    .key = N
menu-close-window =
    .label = Close Window
    .key = W

## Tabs and navigation

tab-new-button =
    .title = Open a new tab
    .aria-label = New tab
tab-close-button =
    .title = Close { $tabTitle }
    .aria-label = Close tab
address-bar =
    .placeholder = Search with { $engine } or enter address
    .aria-label = Address and search bar
back-button =
    .title = Go back one page ({ $shortcut })
    .aria-label = Back
forward-button =
    .title = Go forward one page ({ $shortcut })
    .aria-label = Forward
reload-button =
    .title = Reload current page
stop-button =
    .title = Stop loading this page
home-button =
    .title = { -brand-short-name } Home

## Downloads

download-started = Downloading { $fileName }…
download-progress = { NUMBER($percent, maximumFractionDigits: 1) }% of { $totalSize }
download-rate = { $size } per second — { $timeLeft } remaining
download-finished = { $fileName } finished downloading ✓
download-failed =
    The file “{ $fileName }” could not be downloaded.
    Check your connection, then try again.
download-count = { $count ->
    [0] No downloads
    [one] One download
   *[other] { $count } downloads
}
download-remove =
    .label = Remove from History
    .accesskey = R
download-retry =
    .label = Retry Download
    .accesskey = y

## Accounts and synchronization

sync-sign-in = Sign in to { -sync-brand }
    .title = Continue to your account
sync-greeting = Welcome back, { $userName } 👋
sync-last-run = Last synchronized { DATETIME($timestamp, dateStyle: "medium", timeStyle: "short") }
sync-devices = { $deviceCount ->
    [one] Send this tab to one device
   *[other] Send this tab to { $deviceCount } devices
}
sync-items =
    Bookmarks, history, passwords, and open tabs
    are encrypted before they leave this device.
sync-error =
    .title = Sync temporarily unavailable
    .message = Try again later or visit { -support-site }.
sync-disconnect =
    .label = Disconnect…
    .accesskey = D

## Privacy and permissions

permission-camera = Allow { $host } to use your camera?
permission-microphone = Allow { $host } to use your microphone?
permission-location = Allow { $host } to access your location?
permission-notifications = Allow { $host } to send notifications?
permission-choice = { $permission ->
    [camera] Camera
    [microphone] Microphone
    [location] Location
   *[other] Site permission
}
permission-duration = { $duration ->
    [once] Allow once
    [session] Allow for this session
   *[always] Always allow
}
permission-block =
    .label = Block
    .accesskey = B
permission-allow =
    .label = Allow
    .accesskey = A
tracking-protection = Enhanced Tracking Protection is { $state } for this site.
tracking-protection-toggle =
    .label = Enhanced Tracking Protection
    .aria-label = Toggle protection for { $host }

## Plural categories and nested placeables

open-tabs = { $tabCount ->
    [0] You have no open tabs.
    [one] You have one open tab.
   *[other] You have { NUMBER($tabCount) } open tabs.
}
shared-photos = { $gender ->
    [masculine] { $userName } added { $photoCount } photos to his stream.
    [feminine] { $userName } added { $photoCount } photos to her stream.
   *[other] { $userName } added { $photoCount } photos to their stream.
}
storage-size = { $unit ->
    [byte] { NUMBER($amount) } B
    [kilobyte] { NUMBER($amount, maximumFractionDigits: 1) } KB
    [megabyte] { NUMBER($amount, maximumFractionDigits: 1) } MB
   *[gigabyte] { NUMBER($amount, maximumFractionDigits: 2) } GB
}
session-restore = { $windowCount ->
    [one] Restore one window with { $tabCount } tabs?
   *[other] Restore { $windowCount } windows with { $tabCount } tabs?
}

## Dates, numbers, and literals

history-today = Today — { DATETIME($date, timeStyle: "short") }
history-date = { DATETIME($date, year: "numeric", month: "long", day: "numeric") }
zoom-level = Zoom: { NUMBER($zoom, style: "percent") }
memory-usage = Memory: { NUMBER($megabytes, maximumFractionDigits: 0) } MB
update-version = Version { $version } ({ $buildId })
literal-braces = Example code: { "let object = { key: \"value\" };" }
literal-backslash = Windows path: { "C:\\Users\\Public" }
unicode-sample = Français, Ελληνικά, العربية, हिन्दी, 日本語, 한국어.
astral-sample = Astral symbols: 🦊 🌍 U+1F680 → 🚀
empty-literal = An intentionally empty value follows: { "" }.
fixed-reference = Internal build number { 2026 }.

## Updates and restart flow

update-available = A new { -brand-short-name } update is available.
update-downloading = Downloading update — { NUMBER($percent) }%
update-ready =
    The update will be installed when you restart { -brand-short-name }.
    Save your work before continuing.
update-restart =
    .label = Restart to Update
    .accesskey = R
update-later =
    .label = Not Now
    .accesskey = N
restart-required = { PLATFORM() ->
    [windows] Restart Windows to finish the installation.
    [macos] Restart macOS to finish the installation.
   *[other] Restart your system to finish the installation.
}

## Diagnostics and developer tools

devtools-title = Developer Tools — { $toolName }
devtools-dock = { $position ->
    [bottom] Dock to bottom
    [left] Dock to left
    [right] Dock to right
   *[window] Separate window
}
console-message-count = { $count ->
    [one] One console message
   *[other] { $count } console messages
}
network-request = { $method } { $url } returned { $status }.
network-timing = Completed in { NUMBER($milliseconds, maximumFractionDigits: 2) } ms
source-location = { $fileName }:{ $lineNumber }:{ $columnNumber }
copy-as-curl =
    .label = Copy as cURL
    .accesskey = C
inspector-search =
    .placeholder = Search HTML
    .key = F

# This final message deliberately combines references and punctuation.
about-footer = { -brand-full-name } is made by { -vendor-name } — thanks for testing! ✨
