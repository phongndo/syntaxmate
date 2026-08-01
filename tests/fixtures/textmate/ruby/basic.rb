# frozen_string_literal: true
=begin
Ruby basic fixture: café, 東京, λ, 🚀, and astral 𝌆.
=end
module Aurora
  PATTERN = /\A(?:ready|launch)\z/i
  BANNER = <<~TEXT
    Mission café 東京 🚀 𝌆
  TEXT

  class Greeting
    attr_reader :name, :tags
    def initialize(name:, tags: %i[basic unicode])
      @name = name
      @tags = tags
    end

    def render(count = 1)
      label = case count
              when 0 then "none"
              when 1 then "hello #{@name}"
              else "#{count} launches for #{@name}"
              end
      "#{label}: #{@tags.join(', ')}"
    end
  end
end

greeter = Aurora::Greeting.new(name: "λ")
puts [1, 2].map { |count| greeter.render(count) }
