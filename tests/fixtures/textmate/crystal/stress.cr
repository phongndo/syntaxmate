#!/usr/bin/env crystal
require "json"
require "socket"

# Crystal TextMate stress fixture.
# BMP coverage: café, λ, 東京.
# Astral coverage: 🚀 and 𝌆.
# Every multiline construct below is deliberately closed.

module TextMate
  module Nested::Fixture
    abstract class Base(T)
      abstract def render(value : T) : String
    end

    class Renderer(T) < Base(T)
      getter prefix : String
      property enabled : Bool
      setter callback

      def initialize(@prefix = "東京", @enabled = true)
        @callback = nil
      end

      def render(value : T) : String
        "#{@prefix}: #{value} 🚀"
      end
    end

    struct Point
      property x : Float64
      property y : Float64
    end

    union NumberBox
      value : Int32 | Float64
    end

    annotation Route
    end

    enum State
      Ready
      Running
      Finished
    end
  end
end

lib LibFixture
  type SizeT = UInt64
  alias Callback = Int32 -> Int32
  fun fixture_puts = puts(value : UInt8*) : Int32
end

alias StringMap = Hash(String, String)
record Reading, name : String, value : Float64

class 東京
  @@instances = 0
  CONSTANT_VALUE = 42

  def initialize(@name : String, @𝒜value = "𝒜")
    @@instances += 1
  end

  def name?
    !@name.empty?
  end

  def [](index : Int32)
    @name[index]
  end

  def +(other : self)
    self.class.new(@name + other.to_s)
  end

  def self.count
    @@instances
  end
end

$fixture_global = "global"
%fresh_variable = "fresh"
predefined = [$!, $&, $1, $~, $-v]
environment = ENV["FIXTURE_MODE"]? || "test"
locations = {__DIR__, __FILE__, __LINE__, __END_LINE__}
identity = self
types = {typeof(environment), sizeof(Int64), instance_sizeof(東京)}
pointer = pointerof(environment)
uninitialized_value = uninitialized Int32

numbers = {
  decimal: 1_000_i64,
  hexadecimal: 0xCA_FE_u32,
  octal: 0o755_u16,
  binary: 0b1010_0110_u8,
  ordinary_float: 3.141_592,
  exponent_float: 6.022e23,
  suffixed_float: 1.25_f32,
}

symbols = {
  :plain,
  :'single quoted',
  :"interpolated-#{numbers[:decimal]}",
  :ready?,
  :[]=,
  {label: :hash_key},
}

single_quoted = 'λ'
escaped_quote = '\''
double_quoted = "line\n tab\t hex=\x41 unicode=\u03bb wide=\u{1F680}"
interpolated = "nested #{ {value: {city: "東京"}} } 🚀 𝌆"
command = `printf "#{environment}"`
command_curly = %x{printf #{environment} {nested}}
command_brackets = %x[printf #{environment} [nested]]
command_angles = %x<printf #{environment} <nested>>
command_parens = %x(printf #{environment} (nested))
command_pipes = %x|printf #{environment}|

upper_parens = %Q(alpha (beta) #{environment})
upper_brackets = %Q[alpha [beta] #{environment}]
upper_angles = %Q<alpha <beta> #{environment}>
upper_curly = %Q{alpha {beta} #{environment}}
upper_pipes = %Q|alpha #{environment}|
implicit_upper = %(plain #{environment} (nested))

lower_parens = %q(alpha (beta) \))
lower_brackets = %w[alpha [beta] gamma]
lower_angles = %i<alpha <beta> gamma>
lower_curly = %q{alpha {beta} \}}
lower_pipes = %w|alpha beta\|gamma|

classic_regex = /^(?<word>[[:alpha:]]+)\s+東京{1,3}$/im
condition_regex = if /(?:alpha|beta)[0-9]+/x
                    true
                  else
                    false
                  end
curly_regex = %r{^(alpha){1,3}[0-9]+#{environment}$}ix
bracket_regex = %r[^(alpha)[0-9]+#{environment}$]m
paren_regex = %r(^(alpha)(beta){2,4}#{environment}$)s
angle_regex = %r<^(alpha)<beta>#{environment}$>i
pipe_regex = %r|^(alpha|beta)\s+東京$|i

html = <<-HTML
<section class="fixture">
  <h1>#{environment} 🚀</h1>
</section>
HTML

sql = <<-SQL
SELECT id, name
FROM fixtures
WHERE city = '東京';
SQL

css = <<-CSS
.fixture {
  color: rebeccapurple;
}
CSS

cpp = <<-CPP
#include <string>
std::string label = "fixture";
CPP

c_source = <<-C
#include <stdint.h>
uint32_t answer = 42;
C

javascript = <<-JAVASCRIPT
const city = "東京";
console.log(`${city} 🚀`);
JAVASCRIPT

jquery = <<-JQUERY
$(".fixture").attr("data-city", "東京");
JQUERY

shell = <<-SHELL
city='東京'
printf '%s\n' "$city"
SHELL

crystal_source = <<-CRYSTAL
message = "nested #{environment}"
puts message
CRYSTAL

literal_heredoc = <<-'RAW'
No interpolation: #{environment}
Backslash remains: C:\fixture\東京
RAW

ordinary_heredoc = <<-TEXT
Interpolation works: #{environment}
Unicode remains: λ 東京 🚀 𝌆
TEXT

macro trace(expression)
  {% if flag?(:debug) %}
    puts {{ expression }}
  {% else %}
    {{ expression }}
  {% end %}
end

values = [1, 2, 3, 4]
mapped = values.map do |value, index|
  value ** 2 + index - 1
end
selected = values.select { |value| value >= 2 && value != 4 }
lookup = {"left" => 1, "right" => 2}
range = 1..10
shifted = (numbers[:decimal] << 2) >> 1
flags = (0b0011 & 0b0101) | 0b1000 ^ 0b0001
safe_name = TextMate::Nested::Fixture::Renderer(String).new(&.to_s)

def classify(value)
  case value
  when Nil
    :nil
  when String
    value.empty? ? :empty : :text
  else
    value.responds_to?(:to_s) ? :convertible : :unknown
  end
rescue error : Exception
  raise error
ensure
  puts "classification complete"
end

spawn do
  loop do
    break if selected.empty?
    next unless condition_regex
    yield if false
    break
  end
end

trace(classify(interpolated))
puts html, sql, css, cpp, c_source, javascript, jquery, shell
