{-# LANGUAGE DerivingStrategies #-}
{-# LANGUAGE UnicodeSyntax #-}

module Fixture.Basic (Box (..), Tag (..), Pair, Pretty (..), twice, greet) where

import Data.Char (toUpper)

-- | A tiny record used by the basic syntax fixture.
data Box a = Box {unBox ∷ a}
  deriving stock (Eq, Show)

newtype Tag = Tag String
  deriving stock (Eq, Show)

type Pair a = (a, a)

class Pretty a where
  pretty ∷ a → String

instance Pretty Tag where
  pretty (Tag value) = concat ["tag:", value]

{- An outer comment {- with a nested comment mentioning 東京 -} closes here. -}
twice ∷ (a → a) → a → a
twice f = \x → f (f x)

greet name = concat ["Hello, ", map toUpper name, " — λ 東京 🚀 𝌆\n"]
