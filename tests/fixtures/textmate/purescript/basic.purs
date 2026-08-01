module Fixture.PureScript.Basic (Greeting(Hello, Quiet), Visitor, welcome, main) where

import Prelude
import Effect (Effect)
import Effect.Console as Console

-- A compact café greeting crosses BMP λ and astral 🚀 text safely.
data Greeting = Hello String | Quiet

type Visitor = { name :: String, visits :: Int }

welcome :: Visitor -> Greeting
welcome visitor =
  if visitor.visits > 0 then
    Hello ("Olá, " <> visitor.name <> " 🚀")
  else
    Quiet

render :: Greeting -> String
render greeting = case greeting of
  Hello message -> message
  Quiet -> "Please visit again"

main :: Effect Unit
main = Console.log (render (welcome { name: "Zoë", visits: 2 }))
