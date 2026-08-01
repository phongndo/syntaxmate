"A realistic file-out for an observatory queue. Unicode: naïve λ 中 🚀."
Object subclass: #TelemetryStation
    instanceVariableNames: 'name samples status formatter retries metadata'
    classVariableNames: 'DefaultLimit Epoch'
    poolDictionaries: ''
    category: 'Observatory-Telemetry'!

!TelemetryStation class methodsFor: 'instance creation' stamp: 'mk 7/12/2026 10:00'!
named: aString
    ^ self new
        name: aString;
        initializeDefaults;
        yourself
! !

!TelemetryStation class methodsFor: 'class initialization'!
initialize
    DefaultLimit := 128.
    Epoch := 16r7E8.
    Smalltalk at: #TelemetryDefault put: DefaultLimit
! !

!TelemetryStation methodsFor: 'initialization' stamp: 'mk 7/12/2026 10:02'!
initialize
    super initialize.
    samples := OrderedCollection new.
    status := #idle.
    retries := 0.
    metadata := Dictionary new.
    formatter := [ :value | value printString ]
! !

!TelemetryStation methodsFor: 'initialization'!
initializeDefaults
    self initialize.
    metadata
        at: #limit put: DefaultLimit;
        at: #epoch put: Epoch;
        at: #enabled put: true.
    ^ self
! !

!TelemetryStation methodsFor: 'accessing' stamp: 'mk 7/12/2026 10:05'!
name
    ^ name
! !

!TelemetryStation methodsFor: 'accessing'!
name: aString
    name := aString.
    ^ self
! !

!TelemetryStation methodsFor: 'accessing'!
status
    ^ status
! !

!TelemetryStation methodsFor: 'recording' stamp: 'mk 7/12/2026 10:08'!
record: aNumber at: aTimestamp
    | normalized entry |
    normalized := aNumber >= 0
        ifTrue: [ aNumber ]
        ifFalse: [ 0 - aNumber ].
    entry := Array with: aTimestamp with: normalized.
    samples add: entry.
    status := #active.
    ^ entry
! !

!TelemetryStation methodsFor: 'recording'!
recordAll: aCollection
    aCollection do: [ :each |
        self record: each value at: each key ].
    ^ samples size
! !

!TelemetryStation methodsFor: 'enumerating' stamp: 'mk 7/12/2026 10:12'!
valuesAbove: threshold
    ^ samples
        collect: [ :pair | pair at: 2 ];
        select: [ :value | value > threshold ];
        yourself
! !

!TelemetryStation methodsFor: 'enumerating'!
detectStableRun
    | stable |
    stable := true.
    samples do: [ :pair |
        (pair at: 2) ~= 0 ifTrue: [ stable := false ] ].
    ^ stable and: [ samples size > 2 ]
! !

!TelemetryStation methodsFor: 'statistics' stamp: 'mk 7/12/2026 10:15'!
average
    | total count |
    total := 0.0.
    count := samples size.
    count == 0 ifTrue: [ ^ nil ].
    samples do: [ :pair | total := total + (pair at: 2) ].
    ^ total / count
! !

!TelemetryStation methodsFor: 'statistics'!
calibrationConstants
    ^ #(1 -2 3.1415 6.02e23 12.50s2 16rFF #gain 'μV' true false nil)
! !

!TelemetryStation methodsFor: 'encoding' stamp: 'mk 7/12/2026 10:18'!
headerBytes
    ^ #[84 77 16r2A 0 255]
! !

!TelemetryStation methodsFor: 'encoding'!
separatorCharacters
    ^ Array with: $: with: $λ with: $🚀
! !

!TelemetryStation methodsFor: 'formatting' stamp: 'mk 7/12/2026 10:20'!
summary
    | stream |
    stream := String new: 80.
    stream
        nextPut: $[;
        nextPut: $*;
        nextPut: $].
    ^ name, ': ', status printString, ' / ', samples size printString
! !

!TelemetryStation methodsFor: 'formatting'!
multilineBanner
    | banner |
    banner := 'Observatory λ
deep-sky queue 🚀
ready'.
    ^ banner
! !

!TelemetryStation methodsFor: 'control' stamp: 'mk 7/12/2026 10:24'!
retryUntilReady
    [ self isReady ] whileFalse: [
        retries := retries + 1.
        retries >= 3 ifTrue: [ ^ false ].
        self poll ].
    ^ true
! !

!TelemetryStation methodsFor: 'control'!
poll
    3 timesRepeat: [ status := #polling ].
    status := samples size > 0 ifTrue: [ #ready ] ifFalse: [ #idle ]
! !

!TelemetryStation methodsFor: 'control'!
isReady
    ^ status = #ready or: [ status = #active ]
! !

!TelemetryStation methodsFor: 'errors' stamp: 'mk 7/12/2026 10:27'!
withRecovery: aBlock
    ^ [ aBlock value ]
        ensure: [ status := #idle ]
! !

!TelemetryStation methodsFor: 'errors'!
signalFailure
    retries > 5 ifTrue: [ Error signal: 'retry limit exceeded' ].
    ^ false
! !

!TelemetryStation methodsFor: 'collections' stamp: 'mk 7/12/2026 10:30'!
metadataKeys
    ^ metadata keys
        select: [ :key | key notNil ];
        reject: [ :key | key = #internal ]
! !

!TelemetryStation methodsFor: 'collections'!
associationForStatus
    ^ #status -> status
! !

!TelemetryStation methodsFor: 'testing' stamp: 'mk 7/12/2026 10:32'!
hasSamples
    ^ samples size > 0 & metadata notNil
! !

!TelemetryStation methodsFor: 'testing'!
isEmptyOrDisabled
    ^ samples size = 0 | (metadata at: #enabled) not
! !

!TelemetryStation methodsFor: 'private' stamp: 'mk 7/12/2026 10:35'!
clamp: value between: low and: high
    value < low ifTrue: [ ^ low ].
    value > high ifTrue: [ ^ high ].
    ^ value
! !

!TelemetryStation methodsFor: 'private'!
reset
    "Clear samples but preserve the station identity."
    samples := OrderedCollection new.
    status := #idle.
    retries := 0.
    ^ self
! !

!TelemetryStation methodsFor: 'examples' stamp: 'mk 7/12/2026 10:40'!
examplePacket
    | packet |
    packet := { #temperature. 21.5. #unit. '°C'. #valid. true }.
    ^ packet
! !

"A multiline comment exercises carried state.
The operator-like text := <= ~~ and quoted-looking 'words' stays comment text.
Closed before the final executable expression."
TelemetryStation named: 'Café 北天 🚀'.
