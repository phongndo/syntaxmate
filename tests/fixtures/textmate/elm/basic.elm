module Main exposing (Model, Msg(..), main)

import Html exposing (Html, button, div, text)
import Html.Events exposing (onClick)

-- A tiny Unicode greeting launches the fixture 🚀
type alias Model =
    { greeting : String, count : Int }

type Msg
    = Increment
    | Reset

initialModel : Model
initialModel =
    { greeting = "Héllo, 世界! 🚀", count = 0 }

update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1 }

        Reset ->
            { initialModel | greeting = model.greeting }

main : Html Msg
main =
    div [] [ text initialModel.greeting, button [ onClick Increment ] [ text "Launch" ] ]
