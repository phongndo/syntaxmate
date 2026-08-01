#!/usr/bin/env dyalog
⍝ Basic vector report for café, Αθήνα, 東京, and orbit 🚀 𝌆
Greeting←'Hello, 世界 🌍'
Values←¯3 0 1.5 6.022E23
⎕IO←0
Squares←Values*2
Report←{⍵,' → ',⍕+/⍺}
Message←'sum' Report Squares
Selected←(Values≥0)/Values
First←Selected[0]
:If First=0
    ⎕←"zero begins the non-negative data"
:Else
    ⎕←'unexpected origin'
:EndIf
∇ Total←Add Left Right;Note
  Note←'dyadic function'
  Total←Left+Right
∇
:Class Counter : Object IFormattable,IDisposable
  :Field Public Shared ReadOnly Kind←'basic'
:EndClass
)LOAD demo -quiet
]DISPLAY -format=JSON Message
)OFF ⍝ end the small session
