# language: en (keyword variants below intentionally exercise the grammar)
@observatory @allocation-engine @unicode-🪐
Feature: Coordinate a global robotic observatory network
  Researchers submit campaigns and the scheduler plans stations from Reykjavík to 東京 and القمر.

  Background: A synchronized network
    Given the control plane is available at "https://control.example/α"
    And stations "aurora", "東京-7", and "القمر-2" report healthy
    And the planning epoch is '2042-11-03T23:58:00Z'

  Rule: Campaigns are normalized before allocation
    Every accepted campaign has a bounded window and a safe instrument profile.
    @normalization @quotes
    Scenario: Preserve a scientist's intent
      Given researcher "Ada O'Connell" submits campaign "exoplanet-🛰️"
      And the title is "Transit of \"Kepler-186 f\""
      And the local path is 'D:\\data\\night\'s-run\\raw'
      When the gateway normalizes the request
      Then the normalized request contains:
        | field       | value                         |
        | owner       | Ada O'Connell                 |
        | target      | Kepler-186 f                  |
        | passband    | Hα                            |
        | storage_key | campaigns/2042/exoplanet-🛰️ |
      And the provenance document is:
        """application/json
        {
          "source": "proposal-β",
          "note": "keep \\"quoted\\" intent",
          "target": "Kepler-186 f"
        }
        """
      But no observing slot is allocated yet

    @boundaries
    Scenario Outline: Validate campaign windows
      Given campaign "<campaign>" opens at "<opens>"
      And it closes at "<closes>"
      When validation compares the timestamps
      Then its status is "<status>"
      And the explanation is '<reason>'

      Examples: valid and invalid windows
        | campaign | opens                | closes               | status   | reason                 |
        | polar-1  | 2042-11-04T00:00:00Z | 2042-11-04T01:00:00Z | accepted | positive duration      |
        | lunar-🌕 | 2042-11-04T02:00:00Z | 2042-11-04T02:00:00Z | rejected | empty window           |
        | nova-β   | 2042-11-04T04:00:00Z | 2042-11-04T03:59:59Z | rejected | closes before opening  |

      @leap-second
      Examples: edge timestamps
        | campaign | opens                | closes               | status   | reason                 |
        | edge-60  | 2042-06-30T23:59:59Z | 2042-07-01T00:00:01Z | accepted | crosses UTC midnight   |
        | old-epoch| 1969-12-31T23:59:58Z | 1970-01-01T00:00:00Z | rejected | archive is read only   |

  Rule: Allocation respects weather, priority, and fairness
    The planner may skip an unsafe station but must explain every decision.

    @weather @multilingual
    Scenario: Reroute a campaign around cloud
      Soit station "aurora" has cloud cover 95 percent
      Et station "東京-7" has cloud cover 12 percent
      Quand the planner evaluates campaign "exoplanet-🛰️"
      Alors station "aurora" is excluded as "cloud"
      Et station "東京-7" receives the campaign
      Mais the campaign priority remains "P1"

    @emoji-keywords @telemetry
    Scenario: Explain a thermal safety hold
      😐 instrument "IR-3" is at 78 degrees Celsius
      🎬 the safety controller evaluates its envelope
      🙏 the instrument enters "cooldown"
      😂 telemetry includes sensor "cryostat-🌡️"
      😔 no shutter-open command is emitted
      And the operator sees:
        """text/plain
        HOLD IR-3: temperature 78°C exceeds limit 72°C.
        Retry after two stable readings; incident = thermal-🔥.
        """

    @日本語 @東アジア
    Scenario: Schedule from the Tokyo queue
      前提 観測所 "東京-7" は稼働中
      かつ キューに候補 "星雲-猫🐈" がある
      もし スケジューラが晴天の窓を選ぶ
      ならば 候補 "星雲-猫🐈" を 30 分割り当てる
      但し 校正時間 5 分を予約する

    @한국어 @queue
    Scenario: Keep a calibration ahead of science frames
      조건 장비 "분광기-2"가 준비됨
      그리고 보정 프레임 "cal-α"가 대기 중
      만약 과학 프레임 "은하-🚀"를 계획함
      그러면 "cal-α"가 먼저 실행됨
      하지만 두 작업은 같은 필터 "Hα"를 사용함

    @العربية @handoff
    Scenario: Hand a target to the lunar station
      بفرض المحطة "القمر-2" جاهزة
      و الهدف "سديم الجبار" ظاهر
      عندما يقيّم المخطط نافذة الرصد
      ثم تُحجز ثلاثون دقيقة
      لكن لا يبدأ التعريض قبل المعايرة

    @priority-matrix
    Scenario Outline: Break allocation ties deterministically
      Given "<first>" and "<second>" request station "<station>"
      And both fit the window beginning at "<start>"
      When their priorities are <first_priority> and <second_priority>
      Then campaign "<winner>" is scheduled first
      And campaign "<loser>" remains queued

      Examples: priority wins
        | first    | second   | station | start | first_priority | second_priority | winner   | loser    |
        | transit  | mosaic   | aurora | 01:00 | 1              | 2               | transit  | mosaic   |
        | spectrum | comet-☄️ | 東京-7 | 02:30 | 3              | 1               | comet-☄️ | spectrum |

      Examples: equal priority uses age
        | first   | second | station | start | first_priority | second_priority | winner | loser  |
        | old-α   | new-β  | aurora | 03:00 | 2              | 2               | old-α  | new-β  |
        | dawn-01 | zenith  | القمر-2| 04:10 | 4              | 4               | dawn-01| zenith |

  Rule: Execution records remain auditable
    Commands, measurements, and operator decisions share one immutable timeline.

    @audit @doc-string
    Scenario: Record an interrupted exposure
      Given exposure "exp-000042" is running on station "aurora"
      When wind rises above "18 m/s"
      And the safety controller closes the dome
      Then the timeline contains:
        | offset | actor       | event             | detail             |
        | +00:00 | camera      | exposure-started  | filter=Hα          |
        | +02:17 | anemometer  | threshold-crossed | wind=18.4 m/s      |
        | +02:18 | controller  | dome-closed       | reason=high-wind   |
        | +02:19 | camera      | exposure-aborted  | usable=false       |
      And the signed operator note is:
        """text/markdown
        Operator **Márta** acknowledged alert `wind-ζ`.

        Recovery requires inspection of bay 3 and cable 🧵-7.
        Signature: `marta/ops/2042-11-04`
        """
      Then replaying the timeline yields status "aborted"

    @retention @outline
    Scenario Outline: Apply retention policy by artifact class
      Given artifact "<name>" has class "<class>"
      And it was created <age_days> days ago
      When nightly retention runs at "00:30Z"
      Then the artifact action is "<action>"

      Examples: science and operational artifacts
        | name             | class       | age_days | action          |
        | raw-星-001.fits  | raw-science | 29       | retain          |
        | raw-星-002.fits  | raw-science | 31       | archive         |
        | trace-🧭.json    | telemetry   | 8        | compress        |
        | secret.tmp      | credential  | 1        | destroy         |

  # The final assertion verifies a human-readable network summary.
  @summary
  Scenario: Publish the nightly network report
    Given every station has closed its local observing day
    When the coordinator publishes report "night-2042-11-04"
    Then the report lists 3 stations and 7 completed campaigns
    And it ends with "Clear skies — 晴天 — سماء صافية 🌌"
