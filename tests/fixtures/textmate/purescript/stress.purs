{-| Observatory ingestion and rendering fixture.
It includes naïve café text, the BMP symbols λ and 中, and astral 🪐 safely.
{- Nested block comments remain balanced: operators like -> and :: are inert. -}
-}
module Fixture.PureScript.Stress
  ( Observatory
  , ObservatoryId(ObservatoryId)
  , Reading(Temperature, Humidity, Status, Missing)
  , ReadingRow
  , class Render
  , render
  , class Repository
  , load
  , save
  , appendLabel
  , Pair
  , runProgram
  ) where

import Prelude
import Control.Alt
import Control.Monad (when)
import Data.Array as Array
import Data.Either (Either(Left, Right))
import Data.Maybe (Maybe(Just, Nothing), fromMaybe)
import Data.Newtype (class Newtype, unwrap)
import Data.String hiding (null)
import Effect (Effect)
import Effect.Console as Console
import Prim.Row as Row
import Record as Record
import Type.Proxy (Proxy(Proxy))

-- Selective, qualified, aliased, and hiding imports are represented above.
foreign import data ObservatoryHandle :: Type

foreign import openObservatory
  :: String
  -> Effect ObservatoryHandle

foreign import lookupRaw
  :: ObservatoryHandle
  -> String
  -> Effect String

foreign import nowMillis :: Effect Number
type Label = String

type Coordinates =
  { latitude :: Number
  , longitude :: Number
  }

type Metadata =
  { "displayName" :: String
  , enabled :: Boolean
  , tags :: Array String
  }

type ReadingRow r =
  ( value :: Number
  , unit :: String
  , capturedAt :: Number
  | r
  )

type OpenReading r = { value :: Number, unit :: String | r }

type Observatory =
  { id :: ObservatoryId
  , label :: Label
  , coordinates :: Coordinates
  , metadata :: Metadata
  , readings :: Array Reading
  }

newtype ObservatoryId = ObservatoryId String
derive instance newtypeObservatoryId :: Newtype ObservatoryId _
derive newtype instance eqObservatoryId :: Eq ObservatoryId
derive newtype instance ordObservatoryId :: Ord ObservatoryId
data Reading
  = Temperature Number
  | Humidity Number
  | Status Boolean String
  | Missing
derive instance eqReading :: Eq Reading
data Envelope a
  = Empty
  | Envelope { payload :: a, checksum :: Int }
data Phantom :: Type -> Type
data Phantom a = Phantom
class Render a where
  render :: a -> String
  compact :: a -> String
  compact value = render value

class Monad m <= Repository m where
  load :: ObservatoryId -> m (Maybe Observatory)
  save :: Observatory -> m Unit

instance renderReading :: Render Reading where
  render reading = case reading of
    Temperature value -> "temperature=" <> show value
    Humidity value -> "humidity=" <> show value
    Status true message -> "online: " <> message
    Status false message -> "offline: " <> message
    Missing -> "missing"

instance renderId :: Render ObservatoryId where
  render (ObservatoryId value) = value

else instance renderFallback :: Show a => Render a where
  render = show

newtype instance semigroupObservatoryId :: Semigroup ObservatoryId where
  append (ObservatoryId left) (ObservatoryId right) =
    ObservatoryId (left <> right)

appendLabel :: String -> String -> String
appendLabel left right = left <> " / " <> right

infixr 5 appendLabel as +++
infixl 4 map as <#?>
infix 6 type Tuple as :->

type Pair a b = a :-> b

identityUnicode ∷ ∀ a. a → a
identityUnicode value = value

constrained
  :: forall a
   . (Eq a, Show a)
  => a
  -> a
  -> String
constrained left right =
  if left == right then show left else "different"

getValue :: forall r. { value :: Number | r } -> Number
getValue row = row.value

setValue
  :: forall r
   . Number
  -> { value :: Number | r }
  -> { value :: Number | r }
setValue value row = row { value = value }

insertLabel
  :: forall input output
   . Row.Lacks "label" input
  => Row.Cons "label" String input output
  => Record input
  -> Record output
insertLabel = Record.insert (Proxy :: Proxy "label") "sample"

numericSamples :: Array Number
numericSamples = [ 0.0, 12.5, 6.02e23, 1.0E-9 ]

integerSamples :: Array Int
integerSamples = [ 0, 42, 1_000_000, 0xff, 0XCAFE, 0o755 ]

booleanSamples :: Array Boolean
booleanSamples = [ true, false ]

characterSamples :: Array Char
characterSamples = [ 'A', '\n', '\x03bb' ]

escapedText :: String
escapedText = "tab:\t quote:\" slash:\\ lambda:\x03bb"

multilineText :: String
multilineText = """north λ
middle 中
south 🛰️"""

gappedText :: String
gappedText = "alpha \
  \omega"

operatorSection :: Int -> Int
operatorSection value = ((+) 2) value

backtickCall :: String
backtickCall = "left" `appendLabel` "right"

chooseReading :: Maybe Reading -> Reading
chooseReading candidate = fromMaybe Missing candidate

classify :: Number -> Reading
classify value =
  case value of
    n | n < 0.0 -> Missing
    n | n < 50.0 -> Temperature n
    n -> Humidity n

fetchReading :: ObservatoryHandle -> String -> Effect Reading
fetchReading handle key = do
  raw <- lookupRaw handle key
  timestamp <- nowMillis
  let value = if raw == "" then 0.0 else 21.5
  when (timestamp < 0.0) do
    Console.log "clock anomaly"
  pure (classify value)

fetchPair :: ObservatoryHandle -> Effect { first :: Reading, second :: Reading }
fetchPair handle = ado
  first <- fetchReading handle "temperature"
  second <- fetchReading handle "humidity"
  in { first, second }

mapEnvelope :: forall a b. (a -> b) -> Envelope a -> Envelope b
mapEnvelope transform envelope = case envelope of
  Empty -> Empty
  Envelope record -> Envelope (record { payload = transform record.payload })

foldReadings :: Array Reading -> String
foldReadings readings =
  let rendered = map render readings
      prefix = if Array.null rendered then "none" else "readings"
  in prefix +++ show rendered

updateMetadata :: Observatory -> Observatory
updateMetadata observatory =
  observatory
    { metadata = observatory.metadata
        { enabled = not observatory.metadata.enabled
        , tags = observatory.metadata.tags <> [ "λ", "中", "🪐" ]
        }
    }

recover :: Maybe Reading -> Maybe Reading -> Reading
recover primary secondary = fromMaybe Missing (primary <|> secondary)

unfinishedReading :: Reading
unfinishedReading = ?reading

unfinishedRenderer :: Reading -> String
unfinishedRenderer = ?Fallback

runProgram :: Effect Unit
runProgram = do
  handle <- openObservatory "station-café-🪐"
  pair <- fetchPair handle
  let summary = render pair.first +++ render pair.second
  Console.log summary
  Console.log multilineText
