# Hand-written TalonScript TextMate stress fixture.
# Unicode coverage: café, naïve, 東京, λ, Ж, 🚀, 😀, and 𝌆.
# Header contexts exercise plain values and closed regular expressions.
app: /^(Code|VSCodium)$/
title: /[Rr]eadme.+(talon|fixture)$/
os: linux
tag: user.code
tag and mode: user.command
tag or mode: user.navigation
list: user.project
language: en
code.language: python
-
# Top-level special rules and assignments.
tag(): user.talon_fixture
tag(): user.unicode_commands
settings():
    key_wait = 1.25
    user.command_history = 20
    user.fixture_name = "café 東京 λ 🚀 𝌆"
    user.allow_navigation = 1
key(ctrl-alt-shift-f8):
    app.notify("hotkey 🚀")
key(f9:down):
    user.hold_key("f9")
key(f9:up):
    user.release_key("f9")
key(tab:3):
    app.notify("three tabs")
deck(button_a):
    user.deck_press("button_a")
gamepad(left_trigger:change):
    user.axis_changed("left_trigger")
action(primary):
    user.choose_action("primary")
face(smile:start):
    user.expression_started("smile")
parrot(pop:stop):
    user.sound_stopped("pop")
# Speech rules cover alternatives, anchors, lists, and captures.
(open | show) <user.project_name>:
    key(ctrl-p)
    insert("{project_name}")
    key(enter)
choose {user.project}:
    user.open_project(project)
    app.notify("opened {project}")
^literal anchored command$:
    insert("anchors are scoped")
optional [polite] action:
    app.notify("optional words")
(left | right | center) pane:
    user.focus_pane("chosen")
go to line <number>:
    key(ctrl-g)
    insert("{number}")
    key(enter)
search for <user.text>:
    key(ctrl-f)
    insert("{text}")
    key(enter)
# Nested actions, separators, numbers, and arithmetic operators.
build report:
    user.report.create("daily", 42, 3.5)
    user.report.add(user.lookup("café"), "東京")
    user.report.finish()
compute sample:
    total = 8 + 5
    scaled = total * 2
    ratio = scaled / 4
    remaining = ratio - 1
    fallback = remaining or 7
    app.notify("value {fallback}")
call with many arguments:
    user.combine("alpha", 'beta', 10, 2.75, item_list)
    user.wrap(user.inner("λ"), user.inner("🚀"))
# Strings cover escapes, interpolation, braces, and both quote styles.
quoted strings:
    insert("double quote: \" and slash: \\")
    insert('single quote: \' and tab: \t')
    insert("line one\nline two\rline three")
    insert("literal braces {{ and }}")
    insert("formatted {user.name} and {score + 1}")
multiline string:
    insert("""first line café
second line 東京 λ
third line 🚀 𝌆""")
    app.notify("triple string closed")
adjacent string calls:
    insert("north")
    insert('south')
    insert("east")
    insert('west')
# Key actions exercise prefixes, states, repetitions, and quoted keys.
navigation keys:
    key(home)
    key(shift-end)
    key(ctrl-c)
    key(ctrl-v)
    key(alt-left:2)
    key(cmd-space)
    key(super-tab)
key states:
    key(ctrl:down)
    key(shift:down)
    key('a':repeat)
    key(shift:up)
    key(ctrl:up)
editing sequence:
    key(ctrl-a)
    key(backspace)
    insert("replacement")
    key(enter:2)
# Full-line comments remain valid statements inside command bodies.
document selection:
    # Select everything before applying the fixture transformation.
    edit.select_all()
    user.transform("normalize")
    # The selection is intentionally cleared at the end.
    edit.selection_clear()
notify languages:
    app.notify("café")
    app.notify("東京")
    app.notify("λ")
    app.notify("Ж")
    app.notify("🚀")
    app.notify("😀")
    app.notify("𝌆")
# Dotted action names and variable-shaped arguments.
workspace overview:
    user.workspace.open()
    user.workspace.focus(project_list)
    user.workspace.describe(active_2)
    user.workspace.close()
format identifiers:
    snake = user.formatter.snake_case("sample value")
    camel = user.formatter.camel_case("sample value")
    user.insert_pair(snake, camel)
browser address:
    key(ctrl-l)
    insert("https://example.test/café/東京?launch=🚀")
    key(enter)
terminal command:
    key(ctrl-shift-p)
    insert("Terminal: Create New Terminal")
    key(enter)
    insert("printf 'λ 𝌆\\n'")
    key(enter)
# Repeated but varied command blocks stress line-by-line state changes.
mission start:
    user.mission.set_name("Tokyo café")
    user.mission.set_payload("λ", "𝌆")
    user.mission.launch(3)
mission status:
    status = user.mission.status()
    app.notify("status {status}")
mission pause:
    user.mission.pause()
    app.notify("paused")
mission resume:
    user.mission.resume()
    app.notify("resumed")
mission finish:
    user.mission.finish()
    app.notify("landed 🚀")
list first item:
    user.pick(project_1)
list second item:
    user.pick(project_2)
list final item:
    user.pick(project_list)
# An intentional inline comment exercises the grammar's invalid-comment scope.
inline comment sample:
    app.notify("before comment") # inline comments are not valid Talon statements
    app.notify("after comment")
final save:
    edit.save()
    app.notify("fixture complete: café 東京 λ 🚀 𝌆")
final command:
    insert("all explicit strings and action calls are closed")
