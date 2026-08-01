# frozen_string_literal: true
# encoding: UTF-8

# Ruby TextMate stress fixture: café, λ, 🚀, and the astral symbol 𝌆.
# The examples are intentionally small, but cover stateful and modern syntax.

module Aurora
  VERSION = "2.4.0"
  DEFAULT_TAGS = %i[stable observable unicode].freeze
  ORBITS = { low: 180..2_000, high: 2_001...36_000 }.freeze

  Event = Struct.new(:name, :payload, :at, keyword_init: true) do
    def summary = "#{name}@#{at || 'unknown'}"
  end

  module Formatting
    refine String do
      def command? = match?(/\A(?:run|launch|deploy)[!?]?\z/i)
    end
  end

  class Telemetry
    include Enumerable
    using Formatting

    attr_reader :events
    attr_accessor :enabled

    class << self
      def build(**options, &observer)
        new([], **options, &observer)
      end

      alias create build
    end

    def initialize(events = [], enabled: true, limit: 100, **metadata, &observer)
      @events = events.dup
      @enabled = enabled
      @limit = Integer(limit)
      @metadata = metadata
      @observer = observer || ->(event) { event.summary }
    end

    def each
      return enum_for(__method__) unless block_given?

      @events.each { |event| yield event }
    end

    def add(name, payload = {}, at: Time.now, **extra)
      return :disabled unless enabled

      event = Event.new(name:, payload: payload.merge(extra), at:)
      @events.push(event)
      @events.shift while @events.length > @limit
      @observer&.call(event)
      event
    end

    def classify(message)
      case message
      in { event: "launch", payload: { vehicle:, crew: [captain, *others] } }
        [:mission, vehicle, captain, others.length]
      in { event:, severity: 8..10 } if event.to_s.command?
        [:urgent, event]
      in ["error" | "fatal", Integer => code, *details]
        [:failure, code, *details]
      in { event: }
        [:generic, event]
      else
        [:unknown, message]
      end
    end

    def forward_to(target, ...)
      target.call(...)
    end
  end
end

# Symbols, numeric forms, ranges, and escaped/interpolated strings.
plain_symbol = :ready
quoted_symbol = :"launch-🚀"
dynamic_symbol = :"orbit_#{Aurora::VERSION}"
numbers = [0b1010, 0o755, 0xFF_EC, 1_000_000, 3.14e-2, 2r, 1+2i]
escaped = "tab:\t newline:\n snowman:\u2603 rocket:🚀 glyph:𝌆"
single = 'Interpolation stays literal: #{plain_symbol}.'
adjacent = "Ruby " "concatenates " "these"

# Percent literals use several delimiter families and nested punctuation.
raw_text = %q{literal #{not_interpolated} and nested {braces}}
rich_text = %Q(orbit=#{numbers.fetch(3)}; path=(inner))
words = %w[alpha beta\ gamma delta]
expanded_words = %W[release-#{Aurora::VERSION} café 🚀]
symbols = %i[queued running done]
expanded_symbols = %I[item_#{1 + 1} mission_𝌆]

# Regular expressions include interpolation, named captures, and free spacing.
kind = "mission"
matcher = /\A(?'kind'#{Regexp.escape(kind)}):(?'id'[a-z0-9_-]+)\z/i
path_pattern = %r{(?:api|v\d+)/(?'resource'[\p{L}\p{N}_-]+)}u
extended_pattern = %r{
  \A
  (?'prefix' launch | dock )
  [[:space:]]+
  (?'vehicle' [A-Z][A-Za-z0-9_-]* )
  \z
}ix
replacement = "mission:apollo-🚀".sub(matcher, '\\1/\\2')

# Interpolated and literal heredocs exercise multiline begin/end state.
operator = "Ada"
briefing = <<BRIEFING
  Mission: #{kind.upcase}
  Operator: #{operator}
  Symbols: 🚀 and 𝌆
  Escaped braces: \#{literal_at_runtime}
BRIEFING

query = <<'SQL'
  SELECT mission_id, payload
    FROM telemetry
   WHERE payload LIKE '#{not_sql_interpolation}%'
   ORDER BY mission_id DESC;
SQL

document = <<JSON
  {
    "mission": "#{kind}",
    "active": true,
    "crew": ["Ada", "Matz"],
    "glyph": "𝌆"
  }
JSON

# Blocks, numbered parameters, destructuring, lambdas, and chaining.
telemetry = Aurora::Telemetry.build(limit: 4, source: :fixture) do |event|
  "observed #{event.name}: #{event.payload.keys.join(',')}"
end

missions = %w[apollo gemini artemis]
labels = missions.map.with_index(1) { "#{_2}:#{_1.capitalize}" }
pairs = { launch: 3, dock: 1 }.map { |name, count| [name, count * 2] }
decorate = ->(value, prefix: "★", **) { "#{prefix} #{value}" }
strict_add = lambda { |left, right| left + right }

pipeline = missions
  .lazy
  .map(&:upcase)
  .reject { |name| name.start_with?("G") }
  .take(2)
  .force

labels.zip(pipeline).each do |label, mission_name|
  telemetry.add(:launch, { label:, mission_name: }, priority: 9)
end

# Control flow, modifiers, safe navigation, and exception handling.
result = catch(:complete) do
  3.times do |attempt|
    next if attempt.zero?
    redo if false
    throw :complete, attempt if attempt >= 2
  end
end

status = if telemetry.enabled && !telemetry.events.empty?
           :active
         elsif telemetry&.events&.any?
           :idle
         else
           :offline
         end

case status
when :active, :idle
  puts decorate.call("status=#{status}", prefix: "🚀") unless $VERBOSE
when /^off/
  warn "unexpected string status"
else
  nil
end

begin
  ratio = strict_add.call(20, 2) / Integer(ENV.fetch("DIVISOR", "2"))
rescue ZeroDivisionError
  ratio = Float::INFINITY
rescue ArgumentError, TypeError => error
  ratio = nil
  warn(error.full_message(highlight: false, order: :top))
ensure
  telemetry.enabled = true
end

=begin
This embedded documentation block is intentionally multiline.
It mentions interpolation braces, regular expressions, and heredoc markers.
Unicode remains source text here: naïve façade λ 🚀 𝌆.
=end

puts [briefing, query, document, replacement, raw_text, rich_text].compact.join("\n") if $PROGRAM_NAME == __FILE__
