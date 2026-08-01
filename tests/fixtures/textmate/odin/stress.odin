#+build linux, darwin, !freestanding
package telemetry

import "core:fmt"
import "core:math"
import str "core:strings"
foreign import libc "system:c"
using fmt

App_Name :: "Northwind Telemetry"
// Build-time knobs mirror the collector's wire protocol defaults.
Build_Label :: #config(BUILD_LABEL, "development")
Default_Capacity :: 1 << 10
Binary_Header :: 0b1010_0110
Unix_Mode :: 0o755
Decimal_Budget :: 0d25_000
Magic_Word :: 0xCAFE_BABE
Avogadro :: 6.022_140_76e23
Imaginary_Unit :: 1.0i
Quaternion_J :: 2j
Quaternion_K :: 3k
#assert(Default_Capacity > 0)

Device_ID :: distinct u64
Sequence :: distinct uint
Transform :: matrix[4, 4]f32
Device_State :: distinct u8
State_Unknown :: Device_State(0)
State_Starting :: Device_State(1)
State_Online :: Device_State(4)
State_Sleeping :: Device_State(5)
State_Failed :: Device_State(255)
Capability_Temperature :: 0
Capability_Pressure :: 1
Capability_Position :: 2
Capability_Diagnostics :: 3
Capabilities :: distinct u16
Geo_Point :: [3]f64
Sample_Value :: any
Reading_Buffer :: [dynamic]Sample_Value
Channel_Buffer :: [dynamic]string
Metadata :: map[string]string
Envelope :: [3]any

Reducer :: #type proc "odin" (accumulator, value: f64) -> f64
@(default_calling_convention = "c")
foreign libc {
    @(link_name = "puts")
    c_puts :: proc(message: cstring) -> i32 ---
}
@(private)
registry: map[Device_ID]string
states: map[Device_ID]Device_State
capabilities: map[Device_ID]Capabilities
locations: map[Device_ID]Geo_Point
reading_values: map[Device_ID]Reading_Buffer
reading_channels: map[Device_ID]Channel_Buffer
metadata: map[Device_ID]Metadata
pending: [dynamic]Sample_Value

make_device :: proc(id: Device_ID, allocator := context.allocator) -> Device_ID {
    states[id] = State_Starting
    capabilities[id] = Capabilities((1 << Capability_Temperature) | (1 << Capability_Diagnostics))
    locations[id] = Geo_Point{0, 0, 0}
    reading_values[id] = make(Reading_Buffer, 0, 32, allocator)
    reading_channels[id] = make(Channel_Buffer, 0, 32, allocator)
    metadata[id] = make(Metadata, allocator)
    return id
}
split_coordinate :: proc(point: Geo_Point) -> (latitude, longitude: f64, altitude: f32) {
    return point[0], point[1], point[2]
}
normalize :: proc(value, lower, upper: f64) -> (result: f64, ok: bool) {
    if upper <= lower {
        return 0, false
    }
    result = clamp((value-lower)/(upper-lower), 0, 1)
    return result, true
}
append_sample :: proc(device: Device_ID, channel: string, value: Sample_Value) {
    old_allocator := context.allocator
    context.allocator = context.temp_allocator
    defer context.allocator = old_allocator
    values := reading_values[device]
    channels := reading_channels[device]
    append(&values, value)
    append(&channels, channel)
    reading_values[device] = values
    reading_channels[device] = channels
    states[device] = State_Online
}
value_as_number :: proc(value: Sample_Value) -> (f64, bool) {
    switch item in value {
    case f64:
        return item, true
    case i64:
        return f64(item), true
    case bool:
        return item ? 1.0 : 0.0, true
    case string, Geo_Point:
        return 0, false
    }
    return 0, false
}
describe_any :: proc(payload: any) -> string {
    if point, ok := payload.(Geo_Point); ok {
        return fmt.aprintf("%.2f, %.2f", point[0], point[1])
    }
    return "unsupported payload"
}
fold :: proc(values: []f64, initial: f64, reduce: Reducer) -> f64 {
    result := initial
    for value, index in values {
        result = reduce(result, value)
        if math.is_nan(result) do break
        if index > Default_Capacity do continue
    }
    return result
}
sum :: proc(left, right: f64) -> f64 #force_inline {
    return left + right
}
copy_selected :: proc($T: typeid, source: []T, accept: proc(T) -> bool) -> [dynamic]T {
    result := make([dynamic]T, 0, len(source), context.allocator)
    for element in source {
        if accept(element) {
            append(&result, element)
        }
    }
    return result
}
state_label :: proc(state: Device_State) -> string {
    switch state {
    case State_Unknown:  return "unknown"
    case State_Starting: fallthrough
    case State_Online:   return "active"
    case State_Sleeping: return "sleeping"
    case State_Failed:   return "failed"
    }
    return "invalid"
}

poll_window :: proc(device: Device_ID, first, last: int) {
window: for index in first..<last {
        when ODIN_OS == .Linux {
            if index not_in 0..<Default_Capacity do break window
        } else {
            if index < 0 do continue window
        }
        switch index % 3 {
        case 0:
            append_sample(device, "temperature", 21.5)
        case 1:
            append_sample(device, "pressure", i64(101_325))
        case:
            append_sample(device, "healthy", true)
        }
    }
    for retry in 0..=3 {
        if retry == 3 do fmt.println("poll window complete")
    }
}

format_report :: proc(device: Device_ID) -> string {
    builder: str.Builder
    str.builder_init(&builder, context.allocator)
    defer str.builder_destroy(&builder)
    /* Nested comments exercise balanced grammar state.
       /* The probe remains coherent and deliberately finite. */
       Coordinates are emitted after the heading. */
    heading := "Station 東京 — café observatory 🛰️\n"
    escapes := "tab:\t quote:\" rune:\u03bb ansi:\x1b[32mgreen\x1b[0m"
    raw_layout := `channel | value
---------+------`
    marker := 'λ'
    newline := '\n'
    str.write_string(&builder, heading)
    fmt.sbprintf(&builder, "%s\n%c %c\n", escapes, marker, newline)
    str.write_string(&builder, raw_layout)
    channels := reading_channels[device]
    for value, index in reading_values[device] {
        if number, ok := value.(f64); ok {
            fmt.sbprintf(&builder, "\n%s | %.2f", channels[index], number)
        } else {
            fmt.sbprintf(&builder, "\n%s | %s", channels[index], state_label(states[device]))
        }
    }
    return str.to_string(builder)
}

register_device :: proc(device: Device_ID, name: string) -> bool {
    if existing, present := registry[device]; present {
        fmt.println("already registered:", existing)
        return false
    }
    registry[device] = name
    return true
}
main :: proc() {
    registry = make(map[Device_ID]string)
    states = make(map[Device_ID]Device_State)
    capabilities = make(map[Device_ID]Capabilities)
    locations = make(map[Device_ID]Geo_Point)
    reading_values = make(map[Device_ID]Reading_Buffer)
    reading_channels = make(map[Device_ID]Channel_Buffer)
    metadata = make(map[Device_ID]Metadata)
    pending = make([dynamic]Sample_Value)
    defer delete(registry)
    defer delete(states)
    defer delete(capabilities)
    defer delete(locations)
    defer delete(reading_values)
    defer delete(reading_channels)
    defer delete(metadata)
    defer free(pending)
    station := make_device(Device_ID(Magic_Word))
    register_device(station, "Aster 🚀")
    poll_window(station, 0, 6)
    location := Geo_Point{35.6762, 139.6503, 40}
    locations[station] = location
    latitude, longitude, altitude := split_coordinate(location)
    fmt.printf("position: %.4f, %.4f, %.1f\n", latitude, longitude, altitude)
    values := [?]f64{1, 2.5, 0x10, -4e-1}
    total := fold(values[:], 0, sum)
    ratio, valid := normalize(total, 0, 100)
    assert(valid, "normalization bounds")
    report := format_report(station)
    c_puts(cstring(report))
    fmt.println(Build_Label, ratio, type_of(station), size_of(Geo_Point))
}
