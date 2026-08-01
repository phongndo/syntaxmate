// Basic Dream Maker fixture: café λ 東京 🚀 𝌆
#define FIXTURE_NAME "mark"

/obj/fixture
    var/name = "café λ 東京 🚀 𝌆"
    var/list/tags = list("syntax", "unicode")

    New(label)
        ..()
        if(label)
            name = label

    proc/describe(mob/viewer)
        var/message = {"Fixture: [name]
viewer: [viewer ? viewer : "none"]"}
        return message

    verb/inspect()
        set name = "Inspect fixture"
        usr << describe(usr)

