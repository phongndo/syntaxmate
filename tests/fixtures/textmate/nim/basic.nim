## A compact greeting with café text and a satellite 🛰️.
import std/[strformat, strutils]

type
  Guest* = object
    name*: string
    visits*: Natural

proc welcome*(guest: Guest): string =
  let mood = if guest.visits > 1: "again" else: "today"
  fmt"Hello, {guest.name}, welcome {mood}!"

when isMainModule:
  var guest = Guest(name: "Zoë", visits: 2)
  for rune in ["λ", "中", "🚀"]:
    echo rune.toUpperAscii()
  echo welcome(guest)
