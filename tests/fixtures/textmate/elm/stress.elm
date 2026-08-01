port module Textmate.Stress exposing
    ( Model, Msg(..), Status(..), decodePayload
    , incoming, main, outgoing, shader, (+++)
    )

import Browser
import Dict exposing (Dict)
import Html exposing
    ( Html, button, code, div, h1
    , li, pre, span, text, ul
    )
import Html.Attributes as Attr exposing (class, title)
import Html.Events exposing (onClick)
import Json.Decode as Decode exposing (Decoder)
import Json.Encode as Encode
import Set exposing (Set)
import String

{-| A grammar-oriented but coherent Elm module.

It keeps multiline declarations open, includes nested comments,
and carries Unicode text such as naïve, 東京, λ, and 🚀.

{- Nested block comments exercise recursive begin/end state. -}
-}

type Status
    = Idle
    | Loading Int
    | Ready (List String)
    | Failed String

type alias UserId =
    Int

type alias User =
    { id : UserId
    , name : String
    , tags : Set String
    , preferences :
        { theme : String
        , compact : Bool
        }
    }

type alias Model =
    { user : Maybe User
    , status : Status
    , attempts : Int
    , ratio : Float
    , flags : Dict String Bool
    , note : String
    }

type Msg
    = Begin
    | Received (Result Decode.Error User)
    | Rename String
    | Toggle String
    | NoOp

port outgoing : String -> Cmd msg

port incoming : (String -> msg) -> Sub msg

infixr 5 +++

(+++) : String -> String -> String
(+++) left right =
    left ++ " · " ++ right

emptyUser : User
emptyUser =
    { id = 0
    , name = "Zoë 🚀"
    , tags = Set.fromList [ "elm", "日本語", "façade" ]
    , preferences =
        { theme = "solarized"
        , compact = False
        }
    }

initialModel : Model
initialModel =
    { user = Just emptyUser
    , status = Idle
    , attempts = 0
    , ratio = 6.022e23
    , flags = Dict.fromList [ ( "debug", True ), ( "beta", False ) ]
    , note = "line one\nline two\t\"quoted\""
    }

{- Numbers intentionally cover integer, float, exponent, and hexadecimal forms. -}
numericSamples : List Float
numericSamples =
    [ toFloat 42
    , 3.14159
    , 1e6
    , 2.5E-4
    , toFloat 0x2A
    ]

characterSamples : List Char
characterSamples =
    [ 'a', 'λ', '\n', '\x41' ]

multilineText : String
multilineText =
    """First line: café
Second line: 東京 🚀
Escapes still matter: \t and \"quotes\".
Comment-looking text stays text: {- not a comment -} and -- not one either.
"""

decodeUser : Decoder User
decodeUser =
    Decode.map4 User
        (Decode.field "id" Decode.int)
        (Decode.field "name" Decode.string)
        (Decode.field "tags" (Decode.list Decode.string) |> Decode.map Set.fromList)
        (Decode.field "preferences"
            (Decode.map2
                (\theme compact -> { theme = theme, compact = compact })
                (Decode.field "theme" Decode.string)
                (Decode.field "compact" Decode.bool)
            )
        )

decodePayload : String -> Result Decode.Error User
decodePayload source =
    Decode.decodeString decodeUser source

encodeUser : User -> Encode.Value
encodeUser user =
    Encode.object
        [ ( "id", Encode.int user.id )
        , ( "name", Encode.string user.name )
        , ( "tags", Encode.list Encode.string (Set.toList user.tags) )
        , ( "theme", Encode.string user.preferences.theme )
        ]

update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Begin ->
            ( { model
                | status = Loading (model.attempts + 1)
                , attempts = model.attempts + 1
              }
            , outgoing "begin"
            )

        Received (Ok user) ->
            let
                names =
                    user.tags
                        |> Set.toList
                        |> List.map String.toUpper

                nextStatus =
                    if List.isEmpty names then
                        Ready [ "untagged" ]

                    else
                        Ready names
            in
            ( { model | user = Just user, status = nextStatus }, Cmd.none )

        Received (Err problem) ->
            ( { model | status = Failed (Decode.errorToString problem) }, Cmd.none )

        Rename name ->
            case model.user of
                Just user ->
                    ( { model | user = Just { user | name = name } }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        Toggle key ->
            let
                oldValue =
                    Dict.get key model.flags |> Maybe.withDefault False
            in
            ( { model | flags = Dict.insert key (not oldValue) model.flags }, Cmd.none )

        NoOp ->
            ( model, Cmd.none )

subscriptions : Model -> Sub Msg
subscriptions _ =
    incoming (decodePayload >> Received)

viewStatus : Status -> Html Msg
viewStatus status =
    case status of
        Idle ->
            span [ class "idle" ] [ text "Idle" ]

        Loading n ->
            span [] [ text ("Loading #" ++ String.fromInt n) ]

        Ready values ->
            ul [] (List.map (\value -> li [] [ code [] [ text value ] ]) values)

        Failed reason ->
            pre [ title reason ] [ text reason ]

view : Model -> Html Msg
view model =
    let
        displayName =
            model.user
                |> Maybe.map .name
                |> Maybe.withDefault "anonymous"

        classes =
            [ ( "busy", model.status /= Idle )
            , ( "compact", model.user |> Maybe.map (.preferences >> .compact) |> Maybe.withDefault False )
            ]
    in
    div [ Attr.classList classes ]
        [ h1 [] [ text ("Hello" +++ displayName) ]
        , viewStatus model.status
        , button [ onClick Begin ] [ text "Begin" ]
        , button [ onClick (Toggle "debug") ] [ text "Toggle" ]
        , text multilineText
        ]

shader =
    [glsl|
        precision mediump float;
        uniform vec3 tint;

        void main () {
            gl_FragColor = vec4(tint, 1.0);
        }
    |]

main : Program () Model Msg
main =
    Browser.element
        { init = \_ -> ( initialModel, Cmd.none )
        , update = update
        , subscriptions = subscriptions
        , view = view
        }
