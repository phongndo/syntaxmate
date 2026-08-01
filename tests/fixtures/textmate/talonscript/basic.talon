# Compact TalonScript fixture: café, 東京, λ, 🚀, and 𝌆.
app: code
title: /^(README|notes).+$/
os: linux
tag: user.code
language: en
-
tag(): user.fixture
settings():
    key_wait = 1.5
    user.fixture_label = "café 東京 λ 🚀 𝌆"
(open | show) <user.file_name>:
    key(ctrl-p)
    insert("open {file_name}")
    key(enter)
save {user.document}:
    edit.save()
    app.notify('saved café')
key(ctrl-shift-r):
    user.run("東京", 2 + 3)
deck(button_a):
    user.select_mode("λ 🚀")
finish fixture:
    insert("closed 𝌆")
