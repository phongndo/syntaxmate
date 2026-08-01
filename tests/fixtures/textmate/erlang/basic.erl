-module(basic).
-moduledoc """
# Basic Erlang fixture

Unicode: **λ**, `東京`, [rocket](https://example.test/🚀), and 𝌆.
""".
-export([hello/1, '東京'/0]).
-record(person, {name = <<>>, active = true}).
-type greeting() :: {ok, binary()} | {error, term()}.

%% Atoms, variables, strings, escapes, maps, records, and bit syntax.
-spec hello(#person{}) -> greeting().
hello(#person{name = Name} = Person) when is_binary(Name) ->
    Count = 16#2A + 2#1010 + 3.5e1,
    Note = "λ 東京 🚀 𝌆\n\t\"quoted\"",
    Data = <<Count:16/little-unsigned-integer, Name/binary>>,
    #{status => 'ready-now', person => Person, note => Note, data => Data},
    {ok, <<"hello, ", Name/binary>>};
hello(_) ->
    {error, bad_person}.

'東京'() -> {$λ, $東, $🚀, $𝌆, $\n, $\x41}.
