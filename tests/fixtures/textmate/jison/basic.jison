/* Compact calculator grammar: café, 東京, and rocket 🚀. */
%{
const trace = [];
function number(text) { return Number(text); }
%}
%lex
%%
\s+                   /* skip whitespace */
[0-9]+("."[0-9]+)?\b return 'NUMBER';
"+"                   return '+';
<<EOF>>               return 'EOF';
/lex
%options flex
%start input
%left '+'
%token NUMBER 0x2A "literal"
%%
input
  : expression EOF { return $1; }
  ;
expression
  : NUMBER { $$ = number(yytext); }
  | expression[left] '+' expression[right]
      { $$ = $left + $right; @$ = @left; }
  | error { yyerrok; $$ = 0; }
  ;
%%
module.exports.description = "Unicode calculator λ 🚀";
