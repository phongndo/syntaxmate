"A compact Squeak-style file-out with café telemetry and a 🚀."
Object subclass: #Greeting
    instanceVariableNames: 'name visits'
    classVariableNames: ''
    poolDictionaries: ''
    category: 'Examples-Unicode'!

!Greeting methodsFor: 'initialization' stamp: 'mk 7/12/2026 09:30'!
initialize
    super initialize.
    name := 'Zoë'.
    visits := 0
! !

!Greeting methodsFor: 'printing'!
messageFor: aGuest
    | punctuation |
    visits := visits + 1.
    punctuation := visits > 1 ifTrue: [ '!' ] ifFalse: [ '.' ].
    ^ 'Hello, ', aGuest, ' — λ', punctuation
! !

Greeting new messageFor: '世界 🚀'.
