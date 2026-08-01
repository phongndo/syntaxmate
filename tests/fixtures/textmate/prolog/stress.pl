% Transit observatory fixture: realistic predicates plus broad lexical coverage.
% BMP samples: café, naïve, Ελληνικά, 東京, λ; astral: 🚀 🛰️ 𝌆 🌌.
/*
   The catalogue combines stations, scheduled legs, reports, and a tiny DCG.
   Text such as fake_rule(X) :- write("inside comment"). stays a comment.
*/

:- module(transit_observatory,
          [ route/4,
            itinerary/4,
            station_report/2,
            announce//2,
            run_demo/0
          ]).
:- use_module(library(lists), [append/3, memberchk/2]).
:- dynamic delayed/2.
:- multifile station_note/2.
:- discontiguous station_note/2.
:- meta_predicate guarded(0, -).
:- initialization(run_demo).

% station(Id, DisplayName, ElevationMetres).
station(home, "Home", 18).
station(café, "Café Central", 42).
station(harbor, "Λιμάνι", 3).
station('東京', "東京 Terminal", 27).
station(observatory, "Orbital Observatory 🛰️", 640).
station('deep space', "Deep Space 𝌆", 1200).
station(archive, "Archive 🌌", 11).

zone(home, local).
zone(café, local).
zone(harbor, coast).
zone('東京', international).
zone(observatory, mountain).
zone('deep space', orbital).
zone(archive, local).

% leg(Origin, Destination, Minutes, Mode).
leg(home, café, 4, walk).
leg(café, harbor, 9, tram).
leg(harbor, '東京', 31, ferry).
leg('東京', observatory, 17, rail).
leg(observatory, 'deep space', 88, shuttle).
leg('deep space', archive, 144, capsule).
leg(home, archive, 12, bicycle).
leg(café, observatory, 24, express).
leg(harbor, archive, 16, bus).

fare(walk, 0).
fare(tram, 250).
fare(ferry, 900).
fare(rail, 480).
fare(shuttle, 1250).
fare(capsule, 2048).
fare(bicycle, 75).
fare(express, 700).
fare(bus, 220).

emission(walk, 0.0).
emission(tram, 1.25).
emission(ferry, 8.75).
emission(rail, 2.5).
emission(shuttle, 19.125).
emission(capsule, 13.5).
emission(bicycle, 0.0).
emission(express, 3.75).
emission(bus, 4.5).

station_note(café, 'beans roasted locally').
station_note('東京', 'platform 7 uses 日本語 signs').
station_note(observatory, 'bring a warm coat').
station_note('deep space', 'signal says ''hello'' 🚀').

delayed(ferry, 6).
delayed(capsule, 20).

open_station(Station) :-
    station(Station, _, _),
    \+ closed_station(Station).

closed_station(Station) :-
    memberchk(Station, [maintenance, nowhere]).

direct_route(From, To, Minutes, Mode) :-
    leg(From, To, Minutes, Mode),
    open_station(From),
    open_station(To).

route(From, To, Minutes, [Mode]) :-
    direct_route(From, To, Minutes, Mode).
route(From, To, Minutes, [FirstMode, SecondMode]) :-
    direct_route(From, Via, FirstMinutes, FirstMode),
    direct_route(Via, To, SecondMinutes, SecondMode),
    From \= To,
    Minutes is FirstMinutes + SecondMinutes.

itinerary(From, To, Minutes, Summary) :-
    route(From, To, Minutes, Modes),
    length(Modes, Count),
    Summary = itinerary(From, To, Count, Modes).

acceptable_route(From, To, Limit, Summary) :-
    itinerary(From, To, Minutes, Summary),
    Minutes =< Limit,
    Minutes @> 0.

preferred_route(From, To, Summary) :-
    ( acceptable_route(From, To, 30, Summary) -> true
    ; itinerary(From, To, _, Summary)
    ), !.

same_duration(Left, Right) :-
    Left =:= Right.

ordered_station(Left, Right) :-
    Left @< Right.

not_after(Left, Right) :-
    Left @=< Right.

later_station(Left, Right) :-
    Left @> Right.

duration_delta(Planned, Actual, Delta) :-
    Delta is Actual - Planned.

weighted_duration(Base, Changes, Result) :-
    Result is Base * Changes + 1.

mode_cost(Mode, Riders, Total) :-
    fare(Mode, Unit),
    Total is Unit * Riders.

route_cost([], 0).
route_cost([Mode|Modes], Total) :-
    fare(Mode, Price),
    route_cost(Modes, Rest),
    Total is Price + Rest.

route_emission([], 0.0).
route_emission([Mode|Modes], Total) :-
    emission(Mode, Amount),
    route_emission(Modes, Rest),
    Total is Amount + Rest.

unpack_summary(Summary, Parts) :-
    Summary =.. Parts.

make_summary(From, To, Minutes, Summary) :-
    Parts = [journey, From, To, Minutes],
    Summary =.. Parts.

station_key(Station, Key) :-
    station(Station, Name, Height),
    Key = Height-Name.

station_report(Station, Report) :-
    station(Station, Name, Height),
    ( station_note(Station, Note) -> Detail = Note ; Detail = 'no note' ),
    atom_length(Station, AtomLength),
    Report = report(Name, Height, AtomLength, Detail).

collect_reports(Reports) :-
    findall(Report, station_report(_, Report), Raw),
    sort(Raw, Reports).

collect_modes(Modes) :-
    findall(Mode, leg(_, _, _, Mode), Raw),
    msort(Raw, Ordered),
    sort(Ordered, Modes).

collect_station_keys(Keys) :-
    findall(Key, station_key(_, Key), Raw),
    keysort(Raw, Keys).

station_slug(Station, Slug) :-
    atom_concat('station-', Station, Slug).

station_text(Station, Text) :-
    atom_string(Station, Text).

height_number(Station, Number) :-
    station(Station, _, Height),
    number_string(Height, Text),
    number_string(Number, Text).

bounded_platform(Number) :-
    between(1, 12, Number).

guarded(Goal, Result) :-
    ( catch(Goal, Error, Result = error(Error)) -> Result = ok ; Result = failed ).

safe_report(Station, Result) :-
    guarded(station_report(Station, _), Result).

print_report(Station) :-
    station_report(Station, Report),
    writeq(Report),
    nl.

print_banner :-
    format("~n== Transit café 🚀 ==~n", []).

run_demo :-
    print_banner,
    preferred_route(home, observatory, Summary),
    write_canonical(Summary),
    nl,
    collect_modes(Modes),
    writeln(Modes).

% A small grammar accepts polite journey requests.
announce(From, To) -->
    salutation,
    ["travel"],
    location(From),
    ["to"],
    location(To),
    terminator.

salutation --> ["please"].
salutation --> ["kindly"].
location(home) --> ["home"].
location(café) --> ["café"].
location('東京') --> ["東京"].
location(observatory) --> ["observatory", "🛰️"].
terminator --> ["."].

parse_announcement(Tokens, From, To) :-
    phrase(announce(From, To), Tokens).

sample_tokens(["please", "travel", "home", "to", "東京", "."]).

quoted_forms('north-west', "double quoted", 'astral 🌌').
empty_values([], "", '').
numeric_edges(0, 42, 3.1415, -7).

% Keep Unicode visible on the final physical line: Ω λ 日本語 🚀.
