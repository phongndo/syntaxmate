% A compact route planner with BMP café/東京 and astral 🚀 data.
/* Block comments may mention variables Fake and quoted forms "ignored". */

:- module(route_demo, [journey/3, describe/2, greeting//1]).
:- dynamic closed_station/1.

edge(home, café, 4).
edge(café, '東京', 7).
edge('東京', observatory, 3).
label(observatory, "Orbital deck 🚀").
closed_station(nowhere).

journey(From, To, Cost) :-
    edge(From, Middle, First),
    edge(Middle, To, Second),
    \+ closed_station(Middle),
    Cost is First + Second.

describe(Cost, Message) :-
    ( Cost @> 10 -> Message = "long route" ; Message = "short route" ),
    format("~s (~d)~n", [Message, Cost]), !.

packed(Term, Parts) :- Term =.. [trip, '東京', 14], Parts \= [].
same_cost(Left, Right) :- Left =:= Right.
greeting(Name) --> ["hello"], [Name], ['🛰️'].
