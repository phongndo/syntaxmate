{-# LANGUAGE BinaryLiterals #-}
{-# LANGUAGE DataKinds #-}
{-# LANGUAGE DerivingStrategies #-}
{-# LANGUAGE FlexibleInstances #-}
{-# LANGUAGE FunctionalDependencies #-}
{-# LANGUAGE GADTs #-}
{-# LANGUAGE GeneralizedNewtypeDeriving #-}
{-# LANGUAGE HexFloatLiterals #-}
{-# LANGUAGE KindSignatures #-}
{-# LANGUAGE LambdaCase #-}
{-# LANGUAGE MultiParamTypeClasses #-}
{-# LANGUAGE MultiWayIf #-}
{-# LANGUAGE NamedFieldPuns #-}
{-# LANGUAGE NumericUnderscores #-}
{-# LANGUAGE PatternSynonyms #-}
{-# LANGUAGE RoleAnnotations #-}
{-# LANGUAGE ExplicitForAll #-}
{-# LANGUAGE StandaloneDeriving #-}
{-# LANGUAGE TemplateHaskell #-}
{-# LANGUAGE TypeApplications #-}
{-# LANGUAGE TypeFamilies #-}
{-# LANGUAGE TypeOperators #-}
{-# LANGUAGE UnicodeSyntax #-}
{-# LANGUAGE ViewPatterns #-}
{-# OPTIONS_GHC -Wall #-}

module Fixture.Stress
  ( Name,
    UserId (..),
    Person (..),
    Expr (..),
    Flag (..),
    Toggle,
    pattern Yes,
    pattern No,
    Render (..),
    Convert (..),
    Product (..),
    eval,
    describe,
    workflow,
    unicodeText,
  )
where

import Control.Monad (forM_)
import Data.Char (ord, toUpper)
import Data.Foldable (foldl')
import Data.Kind (Type)
import qualified Data.List as List
import Data.Maybe (fromMaybe)

-- | A short textual name; Unicode examples include λ and 東京.
type Name = String
-- | Identifiers use a derived representation.
newtype UserId = UserId {unUserId ∷ Int}
  deriving newtype (Eq, Ord, Show)
-- ^ A record declaration with named fields.
data Person = Person
  { personName ∷ Name,
    personAge ∷ Int
  }
  deriving stock (Eq, Show)

{-|
A nested block comment exercises balanced lexical regions.

{- The inner comment contains operators: ->, ⇒, <*>, and a rocket 🚀. -}

The outer Haddock block ends normally.
-}
data Flag = On | Off
  deriving stock (Eq, Show)
type family Toggle (flag ∷ Flag) ∷ Flag where
  Toggle 'On = 'Off
  Toggle 'Off = 'On
-- | A small GADT with ordinary, record, and recursive constructors.
data Expr (a ∷ Type) where
  LitInt ∷ Int → Expr Int
  LitBool ∷ Bool → Expr Bool
  Add ∷ Expr Int → Expr Int → Expr Int
  Equal ∷ Eq a ⇒ Expr a → Expr a → Expr Bool
  PairExpr ∷ Expr a → Expr b → Expr (a, b)
deriving instance Show (Expr a)
eval ∷ Expr a → a
eval (LitInt n) = n
eval (LitBool value) = value
eval (Add left right) = sum [eval left, eval right]
eval (Equal left right) = elem (eval left) [eval right]
eval (PairExpr left right) = (eval left, eval right)
class Render a where
  render ∷ a → String
  renderList ∷ [a] → String
  renderList values = concat ["[", List.intercalate ", " (map render values), "]"]
  {-# MINIMAL render #-}
instance Render Person where
  render Person {personName, personAge} = concat [personName, " (", show personAge, ")"]
instance Render UserId where
  render (UserId ident) = '#' : show ident
class Convert a b | a → b where
  convert ∷ a → b
instance Convert UserId String where
  convert (UserId ident) = show ident
type role Product representational representational
data Product a b = Product a b
  deriving stock (Eq, Show)
pattern Yes ∷ Bool
pattern Yes = True
pattern No ∷ Bool
pattern No = False
{-# COMPLETE Yes, No #-}
{-# INLINE greeting #-}
greeting ∷ Person → String
greeting person@Person {personName} =
  concat ["Hello ", personName, "; record=", show person]
birthday ∷ Person → Person
birthday person@Person {personAge} = person {personAge = succ personAge}
describe ∷ Int → String
describe n
  | LT <- compare n 0 = "negative"
  | EQ <- compare n 0 = "zero"
  | Just quotient <- exactHalf n = concat ["twice ", show quotient]
  | otherwise = "positive odd"
  where
    exactHalf value =
      let (quotient, remainder) = value `divMod` 2
       in case remainder of 0 → Just quotient; _ → Nothing
choose ∷ Int → String
choose n =
  if
    | LT <- compare n 10 → "small"
    | LT <- compare n 100 → "medium"
    | otherwise → "large"
logicWord ∷ Bool → String
logicWord = \case
  Yes → "yes"
  No → "no"

sumValues ∷ forall a. Num a ⇒ [a] → a
sumValues = foldl' (\left right → sum [left, right]) 0

workflow ∷ [Name] → IO Int
workflow names = do
  forM_ names (\name →
    putStrLn (map toUpper name)
    )
  let lengths = [length name | name <- names, not (null name)]
  pure (sumValues lengths)

caseStudy ∷ Maybe Person → String
caseStudy candidate =
  case candidate of
    Nothing → "nobody"
    Just Person {personName = "東京", personAge} → concat ["Tokyo:", show personAge]
    Just person → render person

integerLiterals ∷ [Integer]
integerLiterals = [0, 42, 1_000_000, 0xCA_FE, 0o755, 0b1010]

floatingLiterals ∷ [Double]
floatingLiterals = [0.0, 3.141_592, 6.02e23, 0x1.fp3]

characterLiterals ∷ [Char]
characterLiterals = ['λ', '東', '\n', '\o101', '\x6771', '\^A']

escapedText ∷ String
escapedText = "quote=\" slash=\\ tab=\t lambda=\x03bb\&! bell=\BEL"

unicodeText ∷ String
unicodeText =
  "first line: λ 東京\n\
  \second line: 🚀 𝌆"

joinedText ∷ String
joinedText = unwords ["BMP", "λ", "東京", "astral", "🚀", "𝌆"]

readAsInt ∷ Int
readAsInt = read @Int "42"

ordinal ∷ Char → Int
ordinal value = ord value

fallbackName ∷ Maybe Name → Name
fallbackName = fromMaybe "anonymous"

templateValue ∷ Int
templateValue = $([|succ 2|])

{-# WARNING oldGreeting "Use greeting; legacy text mentions 東京 🚀." #-}
oldGreeting ∷ Name → String
oldGreeting name = concat ["Hi, ", name]

-- | Explicit layout and semicolons are also valid Haskell syntax.
compactLet ∷ Int
compactLet = let {x = 1; y = 2} in sum [x, y]

operatorSection ∷ [Int] → [Int]
operatorSection = map succ

compositionExample ∷ Name → String
compositionExample = \value → reverse (map toUpper value)

finalValue ∷ (String, Int)
finalValue = (concat [joinedText, logicWord Yes], compactLet)
