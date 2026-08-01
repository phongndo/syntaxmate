defmodule Mark.Greeter do
  @moduledoc "Small Unicode-aware greeting helpers."
  @default_target :world

  @type target :: atom() | String.t()
  @spec greet(target(), keyword()) :: String.t()
  def greet(target \\ @default_target, opts \\ []) do
    punctuation = Keyword.get(opts, :punctuation, "!")
    label = if target == :world, do: "world", else: to_string(target)
    "Hello, #{label}#{punctuation} 🚀"
  end

  @spec tags(String.t()) :: [String.t()]
  def tags(text) do
    text
    |> String.split(~r/[,;]\s*/u, trim: true)
    |> Enum.map(&String.downcase/1)
  end

  # Atoms, captures, maps, and pattern matching.
  def status(%{ready: true, name: name}), do: {:ok, greet(name)}
  def status(_), do: :error
end
