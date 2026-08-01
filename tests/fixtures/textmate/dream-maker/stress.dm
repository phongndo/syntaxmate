/* = Stress Dream Maker Fixture = */
/*
 * Grammar-driven sample with nested comments.
 * /* nested block comment: café λ 東京 🚀 𝌆 */
 * Every multiline state is closed.
 */
#define FIXTURE_VERSION 3
#define FIXTURE_TITLE "café λ 東京 🚀 𝌆"
#define CLAMP_VALUE(value, low, high) max(low, min(value, high))
#define JOIN_LABEL(left, right...) "[left]: [right]"
#ifdef DEBUG_FIXTURE
#warn Debug fixture enabled
#else
#define DEBUG_FIXTURE 0
#endif
#if 0
/obj/disabled_fixture
    var/message = "This branch is disabled"
#else
/obj/enabled_marker
    var/message = "Enabled preprocessor branch"
#endif
var/global/list/fixture_registry = list()
var/global/const/FIXTURE_LIMIT = 100
var/static/fixture_counter = 0

/datum/fixture_record
    var/id = 0
    var/name = "unnamed"
    var/list/tags = list()
    var/tmp/created_at = 0
    var/const/kind = "record"
    New(new_id, new_name, list/new_tags)
        ..()
        id = new_id
        name = new_name || "record-[new_id]"
        tags = new_tags ? new_tags.Copy() : list()
        created_at = world.time
        fixture_registry += src

    Del()
        fixture_registry -= src
        ..()

    proc/add_tag(tag)
        if(!tag)
            return FALSE
        if(tag in tags)
            return FALSE
        tags += tag
        return TRUE

    proc/remove_tag(tag)
        if(tag in tags)
            tags -= tag
            return TRUE
        return FALSE

    proc/describe()
        var/list/parts = list(
            "id=[id]",
            "name=[name]",
            "tags=[tags.len]"
        )
        return parts.Join(", ")

    proc/as_multiline_text()
        var/text = {"Fixture record
name: [name]
unicode: café λ 東京 🚀 𝌆
tag count: [tags.len]
created: [created_at]"}
        return text

/obj/item/fixture_device
    name = "syntax fixture device"
    desc = "A device named [FIXTURE_TITLE]."
    icon = 'fixture.dmi'
    icon_state = "idle"
    var/datum/fixture_record/record
    var/active = FALSE
    var/charge = 50.0
    var/hex_mask = 0xCAFE

    New(location, label = FIXTURE_TITLE)
        ..()
        fixture_counter++
        record = new /datum/fixture_record(fixture_counter, label, list("device"))

    Destroy()
        if(record)
            del(record)
            record = null
        return ..()

    proc/set_active(value as num)
        active = !!value
        icon_state = active ? "active" : "idle"
        return active

    proc/adjust_charge(delta as num)
        charge = CLAMP_VALUE(charge + delta, 0, 100)
        switch(charge)
            if(0)
                active = FALSE
                return "empty"
            if(1 to 25)
                return "low"
            if(26 to 75)
                return "normal"
            else
                return "full"

    proc/process_ticks(count as num)
        var/total = 0
        for(var/index = 1, index <= count, index++)
            if(!active)
                break
            total += index
            charge -= 0.5
            if(charge <= 0)
                active = FALSE
                break
            else if(index % 10 == 0)
                continue
        return total

    proc/read_registry()
        var/list/results = list()
        for(var/datum/fixture_record/entry in fixture_registry)
            results[entry.id] = entry.describe()
        return results

    proc/find_record(target_id)
        for(var/datum/fixture_record/entry as anything in fixture_registry)
            if(entry.id == target_id)
                return entry
        return null

    proc/wait_for_charge(target)
        while(charge < target)
            charge++
            sleep(1)
        do
            charge--
        while(charge > 100)
        return charge

    proc/schedule_reset(delay)
        spawn(delay)
            active = FALSE
            charge = 50
            world.log << "Reset [src] after [delay] ticks"

    proc/operator_examples(value)
        var/result = (value + 2) * 3 / 4
        result %= 7
        result <<= 1
        result >>= 1
        result ^= hex_mask
        return (result >= 0 && result != 42) || value == null

    proc/string_examples(mob/user)
        var/plain = "Hello, [user ? user.name : "guest"]!"
        var/escaped = "line one\nline two \"quoted\" \[literal bracket]"
        var/resource = 'sounds/fixture.ogg'
        var/long_text = {"A closed multiline string.
It contains interpolation: [record ? record.name : "none"].
It also contains café λ 東京 🚀 𝌆 and ends below."}
        return list(plain, escaped, resource, long_text)

    proc/exception_example()
        try
            if(!record)
                throw EXCEPTION("missing fixture record")
            return record.describe()
        catch(var/exception/error)
            world.log << "Caught: [error]"
            return null

    verb/toggle()
        set name = "Toggle Fixture"
        set category = "Fixtures"
        set src in usr
        active = !active
        usr << "[name] is now [active ? "on" : "off"]."

    verb/show_details()
        set name = "Show Details"
        set desc = "Display café λ 東京 🚀 𝌆"
        set hidden = FALSE
        usr << record?.as_multiline_text()

/mob/fixture_tester
    var/obj/item/fixture_device/device

    Login()
        ..()
        device = new /obj/item/fixture_device(src.loc)
        src << "Welcome, [key]."

    Logout()
        if(device)
            del(device)
        ..()

    verb/run_fixture()
        set name = "Run Syntax Fixture"
        var/list/values = list(1, 2, 3, "four" = 4)
        for(var/value in values)
            world.log << "value=[value]"
        device.set_active(TRUE)
        device.process_ticks(5)
        src << device.record.as_multiline_text()

/world
    name = "Dream Maker Syntax Fixture"
    mob = /mob/fixture_tester
    turf = /turf/fixture_floor
    area = /area/fixture_zone

    New()
        ..()
        world.log << FIXTURE_TITLE

/area/fixture_zone
    name = "Fixture Zone"

/turf/fixture_floor
    name = "fixture floor"
    density = FALSE

// Final procedure ends in ordinary code, outside every comment and string.
/proc/final_fixture_check(value)
    if(isnum(value))
        return value >= 0 ? TRUE : FALSE
    else
        return FALSE
