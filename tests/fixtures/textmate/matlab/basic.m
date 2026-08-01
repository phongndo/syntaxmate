%% Basic MATLAB grammar fixture — 東京 🚀
% Scalars, arrays, continuations, comments, and command syntax.
radius = 3.5;
area = pi * radius ^ 2;
labels = ["naïve", "λ", "orbit 🚀"];
message = 'It''s 100%% ready\n';
values = sum( ... % continue inside a parenthesized expression
    [radius, area, eps]);
%{
This block comment spans lines and carries Ω and 𝌆.
%}
threshold = 25;
if area >= threshold && true
    sample = values' + values.';
elseif radius == 0
    sample = NaN;
else
    sample = Inf;
end
for label = labels
    label = label + "!";
end
disp basic-fixture-ready % command-form call
