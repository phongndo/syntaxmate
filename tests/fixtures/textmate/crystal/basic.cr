#!/usr/bin/env crystal
require "json"

# Basic TextMate fixture: BMP λ 東京; astral 🚀 𝌆.
module Fixture
  class Greeter
    getter name : String
    @@count = 0

    def initialize(@name : String = "東京")
      @@count += 1
    end

    def greet(times = 2)
      tags = %w(λ 東京)
      times.times do |index|
        puts "#{index}: #{@name} 🚀 𝌆"
      end
      {name: @name, tags: tags, ready: true, missing: nil}
    end
  end
end

pp Fixture::Greeter.new.greet(0x2a_i32)
