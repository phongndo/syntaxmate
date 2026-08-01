version 18.0
clear all
set more off

* Small survey fixture: café, 東京, Δ, and 🚀.
global project "orbital-survey"
local years "2024 2025"
local title `"Survey for `years': "ready""'
tempvar centered

sysuse auto, clear
generate double `centered' = price - 5000
replace `centered' = . if missing(price)
label variable `centered' "Centered price — Δ"
summarize price `centered', detail

foreach year of numlist 2024/2025 {
    if (`year' == 2025) {
        quietly count if foreign == 1
    }
    else {
        display as text `"Cohort `year': café 🚀"'
    }
}

assert _N > 0 & _rc == 0
display as result "`title' / $project"
