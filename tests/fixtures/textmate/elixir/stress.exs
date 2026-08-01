#!/usr/bin/env elixir
defmodule Mark.QualityPlan do
  @moduledoc """
  Reviewable examples for TextMate coverage.

  The corpus keeps Unicode such as café, λ, and the astral rocket 🚀.
  """
  alias __MODULE__.Reading
  require Logger
  import Bitwise
  import Kernel, only: [is_number: 1, to_string: 1]
  @typedoc "A stable sensor identifier."
  @type sensor_id :: atom() | String.t()
  @type reading :: %Reading{sensor: sensor_id(), value: number(), tags: [atom()]}
  @type result(value) :: {:ok, value} | {:error, term()}
  @typep sample :: {sensor_id(), number()}
  @opaque token :: reference()
  @callback publish(reading()) :: :ok | {:error, atom()}
  @macrocallback normalize(Macro.t()) :: Macro.t()
  @optional_callbacks publish: 1
  @default_tags [:quality, :"night shift", :'café']
  @limits %{soft: 75.0, hard: 0x64, binary: 0b111_1000, octal: 0o144}
  @message "ready"
  defmodule Reading do
    @enforce_keys [:sensor, :value]
    defstruct sensor: nil, value: 0.0, tags: [], metadata: %{}

    @type t :: %__MODULE__{
            sensor: Mark.QualityPlan.sensor_id(),
            value: number(),
            tags: [atom()],
            metadata: map()
          }
  end
  defprotocol Renderable do
    @fallback_to_any true
    @spec render(t()) :: String.t()
    def render(value)
  end
  defimpl Renderable, for: Any do
    def render(value), do: inspect(value, pretty: true)
  end
  defguard is_safe(value) when is_number(value) and value >= 0 and value <= 100
  defguardp is_named(sensor) when is_atom(sensor) or is_binary(sensor)
  defmacro twice(expression) do
    quote do
      unquote(expression) + unquote(expression)
    end
  end
  @doc "Build a reading with defaults and keyword options."
  @spec new(sensor_id(), number(), keyword()) :: result(reading())
  def new(sensor, value, opts \\ []) when is_named(sensor) and is_safe(value) do
    tags = Keyword.get(opts, :tags, @default_tags)
    metadata = Keyword.get(opts, :metadata, %{source: :fixture})
    {:ok, %Reading{sensor: sensor, value: value, tags: tags, metadata: metadata}}
  end
  def new(sensor, value, _opts), do: {:error, {:invalid, sensor, value}}
  @spec parse(sample()) :: reading()
  def parse({sensor, value}) do
    %Reading{sensor: sensor, value: value, tags: @default_tags}
  end

  def parse(%{"sensor" => sensor, "value" => value} = payload) do
    %Reading{sensor: sensor, value: value, metadata: Map.drop(payload, ["sensor", "value"])}
  end

  def parse(<<kind::8, value::float-64, rest::binary>>) do
    %Reading{sensor: :"channel_#{kind}", value: value, metadata: %{rest: rest}}
  end

  @doc ~s|Classify #{inspect(@limits)} without hiding interpolation.|
  def classify(%Reading{value: value}) when value >= @limits.hard, do: :critical
  def classify(%Reading{value: value}) when value >= @limits.soft, do: :warning
  def classify(%Reading{}), do: :normal

  def summarize(readings) when is_list(readings) do
    readings
    |> Stream.reject(&is_nil/1)
    |> Enum.map(fn %Reading{sensor: sensor, value: value} ->
      "#{sensor}=#{Float.round(value / 1, 2)}°"
    end)
    |> Enum.join(" · ")
  end

  def select(readings, minimum \\ 0) do
    for %Reading{sensor: sensor, value: value} <- readings,
        value >= minimum,
        into: %{} do
      {sensor, value}
    end
  end

  def compare(left, right) do
    cond do
      left === right -> :identical
      left > right and right >= 0 -> {:greater, left - right}
      left < right or left != right -> {:different, abs(left - right)}
      true -> nil
    end
  end

  def merge(left, right) do
    list = left.tags ++ right.tags -- [:discarded]
    bits = (1 <<< 4) ||| (3 &&& 7)
    %{left | tags: Enum.uniq(list), metadata: %{bits: bits, pair: {left, right}}}
  end

  def fetch(config) do
    with {:ok, endpoint} <- Map.fetch(config, :endpoint),
         true <- String.starts_with?(endpoint, "https://") do
      {:ok, endpoint <> "/v1/readings"}
    else
      :error -> {:error, :missing_endpoint}
      false -> {:error, :insecure_endpoint}
    end
  end

  def guarded(fun) when is_function(fun, 0) do
    try do
      {:ok, fun.()}
    rescue
      error in ArgumentError -> {:error, Exception.message(error)}
    catch
      :exit, reason -> {:error, {:exit, reason}}
      kind, value -> {:error, {kind, value}}
    after
      Logger.debug("guarded callback complete")
    end
  end

  def await(timeout \\ 25) do
    receive do
      {:reading, %Reading{} = reading} -> {:ok, reading}
      {:stop, reason} -> {:error, reason}
    after
      timeout -> {:error, :timeout}
    end
  end
  # Captures and pinned variables exercise compact function syntax.
  def captures(values) do
    square = &(&1 * &1)
    formatter = &Renderable.render/1
    pinned = hd(values)
    rendered = case hd(values) do ^pinned -> formatter.(pinned) end
    {Enum.map(values, square), rendered}
  end
  def characters do
    [?a, ?\n, ?\x41, ?λ, ?🚀]
  end
  ## Numeric literal forms used by the grammar.
  def numbers do
    [1_000, 3.141_592e+2, 0xCA_FE, 0b1010_0110, 0o755]
  end

  def strings(name) do
    atom = :"operator => #{name}"
    charlist = 'hello #{name}\n'
    literal = ~S(raw #{name} \\ path)
    words = ~w(alpha beta #{name})a
    regex = ~r/^(?<word>[[:alpha:]]+)\s+#{name}$/iu
    path = ~s{/tmp/#{name}/café}
    {atom, charlist, literal, words, regex, path}
  end

  def heredocs(name) do
    interpolated = """
    hello #{name}
    nested #{inspect(%{person: %{name: name}})} 🚀
    """

    literal = ~S"""
    no interpolation: #{name}
    backslash stays: C:\quality\fixture
    """

    chars = '''
    charlist for #{name}
    '''

    {interpolated, literal, chars}
  end

  def template(assigns) do
    ~H"""
    <section class="quality" data-state={@message}>
      <h2>Sensor <%= assigns.sensor %> 🚀</h2>
      <p title="café">Value: <strong><%= assigns.value %></strong></p>
    </section>
    """
  end

  def legacy_template(assigns) do
    ~L"""
    <article id="legacy"><%= assigns.title %></article>
    """
  end

  def operators(a, b) do
    arithmetic = a + b * 2 - div(a, 3) / 4
    ranges = Enum.to_list(1..10//2)
    joined = [a] ++ [b] |> Enum.reverse()
    logic = (a >= b && b !== 0) || not false
    {arithmetic, ranges, joined, logic, a ** 2, rem(a, 2)}
  end

  def permissions(%{role: role} = user) do
    case {role, Map.get(user, :active, false)} do
      {:admin, true} -> {:ok, [:read, :write, :delete]}
      {role, true} when role in [:editor, :author] -> {:ok, [:read, :write]}
      {:guest, _} -> {:ok, [:read]}
      {_unknown, false} -> {:error, :inactive}
    end
  end

  def private_token(seed), do: make_ref() |> then(fn ref -> {seed, ref} end)
  defp normalize_tag(tag), do: tag |> to_string() |> String.trim() |> String.downcase()
  def normalize_tags(tags) do
    tags |> Enum.map(&normalize_tag/1) |> Enum.reject(&(&1 == ""))
  end
end
