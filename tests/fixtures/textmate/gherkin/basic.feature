# A compact checkout contract with Unicode: café, 東京, and 🛰️
@checkout @smoke-fast
Feature: Reserve a shared observatory session
  Members coordinate scarce telescope time across time zones.

  Background: An authenticated member
    Given member "Zoë 🚀" has 120 credits
    And the observatory clock is set to '23\'59 UTC'

  Rule: A reservation must fit an available window

    @happy_path @station-α
    Scenario: Reserve the last open slot
      Given slot "東京-7" is open from "23:30" to "00:15"
      When Zoë reserves the slot with note "bring \"red\" filter"
      😂 the request includes calibration "β-2"
      Then the reservation is confirmed
      And the receipt contains:
        | field       | value             |
        | member      | Zoë 🚀            |

    @outline @nightly
    Scenario Outline: Reject an unsuitable request
      前提 slot "<slot>" has <available> free minutes
      When Zoë requests <requested> minutes
      Then the result is "<result>"

      Examples: capacity boundaries
        | slot    | available | requested | result   |
        | moon-🌕 | 30        | 45        | too long |
