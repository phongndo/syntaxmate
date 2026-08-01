-module(stress).
-behaviour(gen_server).

-moduledoc """
# Erlang TextMate stress fixture

This module exercises **emphasis**, _italics_, and `inline_code()`.

> Unicode remains visible: λ, 東京, 🚀, and 𝌆.

- [API reference](https://example.test/docs?q=erlang)
- Lists, records, maps, binaries, and processes

```erlang
stress:start_link().
stress:classify(42).
```
""".
-export([start_link/0, stop/0, classify/1, parse/1, transform/2,
         collect/1, comprehensions/2, chars/0, strings/0, sigils/0,
         records/1, maps/1, callbacks/0, maybe_lookup/2, '東京'/0]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2,
         terminate/2, code_change/3]).
-export_type([id/0, result/1, user/0]).
-import(lists, [reverse/1]).
-ifndef(ERLANG_STRESS_HRL).
-define(ERLANG_STRESS_HRL, true).
-define(DEFAULT_TIMEOUT, 1_000).
-define(TAG(Name), {fixture_tag, Name}).
-define(STRINGIFY(Value), ??Value).
-ifdef(TEST).
-define(MODE, test).
-else.
-define(MODE, production).
-endif.
-endif.
-type id() :: non_neg_integer().
-type result(T) :: {ok, T} | {error, term()}.
-opaque token() :: reference().
-record(user, {
    id :: id(),
    name = <<"anonymous">> :: binary(),
    tags = [] :: [atom()],
    meta = #{} :: map()
}).
-type user() :: #user{}.
-doc """
Starts the fixture server.

Returns `{ok, Pid}`; see [gen_server](https://erlang.org/doc/man/gen_server.html).
""".
-spec start_link() -> {ok, pid()} | {error, term()}.
start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, [], []).
stop() ->
    gen_server:stop(?MODULE).
init([]) ->
    {ok, #{mode => ?MODE, counter => 0, users => #{}}}.
handle_call({put, #user{id = Id} = User}, _From, State) ->
    Users0 = maps:get(users, State),
    Users1 = Users0#{Id => User},
    {reply, {ok, Id}, State#{users := Users1}};
handle_call({get, Id}, _From, #{users := Users} = State)
        when is_integer(Id), Id >= 0 ->
    {reply, maps:find(Id, Users), State};
handle_call(Request, _From, State) -> {reply, {error, {unknown_call, Request}}, State}.
handle_cast(increment, #{counter := N} = State) -> {noreply, State#{counter := N + 1}};
handle_cast(_Message, State) -> {noreply, State}.
handle_info({timeout, Ref, ?TAG(cleanup)}, State) when is_reference(Ref) -> {noreply, State};
handle_info(_Info, State) -> {noreply, State}.
terminate(_Reason, _State) -> ok.
code_change(_OldVersion, State, _Extra) -> {ok, State}.
%% Numbers cover decimal, floats, separators, and several bases.
-spec classify(term()) -> atom() | tuple().
classify(N) when is_integer(N), N >= 0, N =< 16#FF ->
    {small_integer, N, 2#1010, 8#755, 16#CAFE, 36#Z, 1_000_000};
classify(N) when is_float(N); is_number(N) ->
    {number, N, 6.022e23, 1.0E-9};
classify(Bin) when is_binary(Bin) -> binary;
classify([]) -> empty_list;
classify([_ | _]) -> list;
classify(Value) when is_map(Value) -> map;
classify(_) -> other.
%% Strings include grammar-recognized escapes and format placeholders.
strings() ->
    ["plain", "λ 東京 🚀 𝌆", "quote=\" slash=\\",
     "line\n tab\t return\r bell\b", "hex=\x41 octal=\101",
     "format: ~p ~10.2f ~ts ~~"].
chars() ->
    {$a, $\n, $\t, $\x41, $\101, $λ, $東, $🚀, $𝌆}.
%% Binary segments exercise size, signedness, endian, unit, and Unicode types.
-spec parse(binary()) -> result(map()).
parse(<<Version:8, Flags:4, Length:12/big-unsigned-integer,
        Payload:Length/binary, Tail/bitstring>>) ->
    Utf = <<$λ/utf8, $東/utf8, 16#1F680/utf32, 16#1D306/utf32>>,
    {ok, #{version => Version, flags => Flags, payload => Payload,
           tail => Tail, unicode => Utf}};
parse(<<Value:16/little-signed-integer-unit:8>>) ->
    {ok, #{value => Value}};
parse(_) ->
    {error, malformed_binary}.
-spec transform([integer()], integer()) -> [integer()].
transform(Values, Limit) ->
    Mapper = fun(X) when X rem 2 =:= 0 -> X div 2;
                (X) -> X * 3 + 1
             end,
    Filter = fun erlang:is_integer/1,
    Local = fun helper/1,
    reverse([Local(Mapper(X)) || X <- Values,
                                Filter(X),
                                X > 0 andalso X =< Limit]).
helper(X) -> (X bsl 1) band 16#FFFF.

-spec comprehensions(binary(), [{term(), term()}]) -> tuple().
comprehensions(Bin, Pairs) ->
    Bytes = << <<(X bxor 16#20)>> || <<X>> <= Bin, X >= $A, X =< $Z >>,
    Map = #{K => V || {K, V} <- Pairs, K =/= undefined},
    Set = [{I, I * I} || I <- lists:seq(1, 8), I rem 2 == 0],
    {Bytes, Map, Set}.

-spec collect(timeout()) -> term().
collect(Timeout) when is_integer(Timeout), Timeout >= 0 ->
    receive
        {From, {echo, Message}} when is_pid(From) ->
            From ! {self(), Message},
            {ok, Message};
        stop ->
            stopped;
        {'EXIT', Pid, Reason} ->
            {exit, Pid, Reason}
    after Timeout ->
        timeout
    end.

-spec records(#user{}) -> tuple().
records(#user{id = Id, name = Name} = User) ->
    Updated = User#user{name = <<Name/binary, "!">>, tags = [seen]},
    {Updated#user.id, #user.name, Id, Updated}.

-spec maps(map()) -> map().
maps(#{required := Value} = Input) ->
    Defaults = #{atom_key => true, 'quoted-key' => false, 1 => one},
    maps:merge(Defaults, Input#{required := {seen, Value}, new_key => '𝌆'}).

-spec callbacks() -> {fun((integer()) -> integer()), function(), function()}.
callbacks() ->
    Named = fun Loop(0, Acc) -> Acc;
                Loop(N, Acc) when N > 0 -> Loop(N - 1, N + Acc)
            end,
    {fun(X) -> Named(X, 0) end, fun lists:reverse/1, fun helper/1}.

-spec maybe_lookup(term(), map()) -> result(term()).
maybe_lookup(Key, Map) ->
    maybe
        {ok, Value} ?= maps:find(Key, Map),
        true ?= Value =/= undefined,
        {ok, Value}
    else
        error -> {error, not_found};
        false -> {error, undefined_value}
    end.

-spec safe_integer(binary()) -> result(non_neg_integer()).
safe_integer(Binary) ->
    try binary_to_integer(Binary) of
        N when N >= 0 -> {ok, N};
        N -> {error, {negative, N}}
    catch
        error:badarg:Stack -> {error, {bad_integer, Stack}};
        Class:Reason -> {error, {Class, Reason}}
    after
        ok
    end.

-spec choose(term()) -> atom().
choose(Value) ->
    case classify(Value) of
        {small_integer, _, _, _, _, _, _} -> small;
        number -> numeric;
        Kind when Kind =:= binary; Kind =:= list -> sequence;
        _ -> unknown
    end.

-spec conditional(integer()) -> atom().
conditional(N) ->
    if
        N < 0 -> negative;
        N == 0 -> zero;
        N > 0 andalso N < 10 -> small;
        true -> large
    end.

%% OTP 27 sigils and triple-quoted strings are explicit grammar branches.
-spec sigils() -> tuple().
sigils() ->
    Escaped = ~"line\n~p",
    Binary = ~b"λ 東京 🚀 𝌆",
    Verbatim = ~B"raw \\n ~p",
    Triple = """
        closed triple string with ~p and Unicode λ 🚀
        """,
    {Escaped, Binary, Verbatim, Triple}.

'東京'() ->
    {'λ', '東京', '🚀', '𝌆', ?STRINGIFY(unicode_atom)}.
