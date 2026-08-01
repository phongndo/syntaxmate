### Core strings for the Borealis browser 🌌

## Welcome and navigation

# Shown on the first-run page.
-brand-short-name = Borealis
welcome = Welcome, { $user }! Explore the web 🌍
    .title = Welcome to { -brand-short-name }
menu-open =
    .label = Open File…
    .accesskey = O
inbox-count = { $count ->
    [one] One new message
   *[other] { $count } new messages
}
updated-at = Last updated { DATETIME($date, dateStyle: "long") }.
account-balance = Balance: { NUMBER($amount, minimumFractionDigits: 2) } { $currency }

## Platform-specific help

quoted-example = Escaped text: { "She said \"hello\"; path C:\\Temp" } — café 東京.
platform-help = { PLATFORM() ->
    [windows] Press Alt to show the menu bar.
    [macos] Press ⌘ to open the menu.
   *[other] Open the application menu.
}
