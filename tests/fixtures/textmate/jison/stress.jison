/*
 * Observatory query grammar used as a broad TextMate fixture.
 * Unicode samples: café, λ, 東京, and astral telescope 🔭 / rocket 🚀.
 */
%{
"use strict";
const DEFAULT_LIMIT = 100;
const units = new Map([["ms", 1], ["s", 1000]]);
function node(type, fields, location) {
  return { type, ...fields, location };
}
function binary(operator, left, right, location) {
  return node("BinaryExpression", { operator, left, right }, location);
}
%}
%lex
%options flex case-insensitive
digit       [0-9]
identifier  [A-Za-z_][A-Za-z0-9_-]*
%s TEMPLATE
%x COMMENT
%%
\s+                         /* discard spacing */
"//"[^\n]*                  /* discard a line comment */
"/*"                        this.pushState('COMMENT');
<COMMENT>"*/"               this.popState();
<COMMENT>.|\n                /* consume comment content */
"`"                         this.pushState('TEMPLATE'); return 'BACKTICK';
<TEMPLATE>"`"               this.popState(); return 'BACKTICK';
<TEMPLATE>"${"              return 'INTERPOLATION_START';
<TEMPLATE>[^`$]+             return 'TEXT';
{digit}+("."{digit}+)?\b    return 'NUMBER';
"true"|"false"              return 'BOOLEAN';
"null"                      return 'NULL';
"let"                       return 'LET';
"where"                     return 'WHERE';
"select"                    return 'SELECT';
"order"                     return 'ORDER';
"by"                        return 'BY';
"asc"                       return 'ASC';
"desc"                      return 'DESC';
"and"                       return 'AND';
"or"                        return 'OR';
"not"                       return 'NOT';
"=="                        return 'EQ';
"!="                        return 'NE';
">="                        return 'GE';
"<="                        return 'LE';
"&&"                        return 'AND';
"||"                        return 'OR';
{identifier}                return 'IDENTIFIER';
\"([^\\\"]|\\.)*\"          return 'STRING';
"("                         return '(';
")"                         return ')';
"["                         return '[';
"]"                         return ']';
","                         return ',';
":"                         return ':';
";"                         return ';';
"+"                         return '+';
"-"                         return '-';
"*"                         return '*';
"/"                         return '/';
"="                         return '=';
">"                         return '>';
"<"                         return '<';
<<EOF>>                     return 'EOF';
/lex
// Generator and parser declarations cover supported directive families.
%options
  flex = true
  locations = true
  module-name = "ObservatoryParser"
  token-stack = false

%include "shared/ast-helpers.js"
%include shared/diagnostics.js
%import
%debug
%parser-type "lalr"
%code init { yy.events = []; }
%code required {{ yy.required = true; }}
%code helper -> yy.normalize = value => String(value).trim()
%parse-param context
%ebnf
%start program
%left OR
%left AND
%nonassoc EQ NE '<' '>' LE GE
%left '+' '-'
%left '*' '/'
%right NOT UMINUS
%token <value> NUMBER 42 0xCAFE
%token STRING IDENTIFIER BOOLEAN NULL
%token LET WHERE SELECT ORDER BY ASC DESC
%token AND OR NOT EQ NE LE GE
%fixture-extension "intentionally-unimplemented"
17 "declaration literal with escape \u03bb"
%%
program
  : statement_list EOF
      {{
        $$ = node("Program", { body: $1 }, @$);
        yy.events.push({ kind: "complete", size: $1.length });
        return $$;
      }}
  | error EOF
      { yyclearin; yyerrok; $$ = node("Program", { body: [] }, @$); }
  ;
statement_list
  : %empty
      { $$ = []; }
  | statement_list statement
      -> $1.concat([$2])
  ;

statement
  : LET IDENTIFIER '=' expression ';'
      { $$ = node("LetStatement", { name: $2, value: $4 }, @$); }
  | query ';'
      %{ $$ = $1; yy.events.push({ kind: "query", at: @1 }); %}
  | %include "shared/statements.jison"
  | error ';'
      { yyerrok; $$ = node("InvalidStatement", { token: #1 }, @$); }
  ;

query
  : SELECT projection WHERE expression order_clause
      {
        $$ = node("Query", {
          projection: $projection,
          predicate: $expression,
          order: $order_clause
        }, @$);
      }
  ;

projection
  : '*'
      { $$ = [{ type: "Wildcard" }]; }
  | expression_list
      { $$ = $1; }
  ;

expression_list
  : expression[item]
      { $$ = [$item]; }
  | expression_list[items] ',' expression[item]
      { $$ = $items.concat([$item]); }
  ;

order_clause
  : %epsilon
      { $$ = null; }
  | ORDER BY IDENTIFIER direction
      { $$ = { field: $3, direction: $4 }; }
  | ORDER BY IDENTIFIER error
      { yyerrok; $$ = { field: $3, direction: "asc" }; }
  ;

direction
  : ASC { $$ = "asc"; }
  | DESC { $$ = "desc"; }
  | ɛ { $$ = "asc"; }
  ;

expression
  : literal
      { $$ = $1; }
  | IDENTIFIER
      { $$ = node("Identifier", { name: $1 }, @1); }
  | '(' expression ')'
      -> $2
  | '[' expression_list ']'
      { $$ = node("ArrayExpression", { elements: $2 }, @$); }
  | NOT expression
      %prec NOT { $$ = node("UnaryExpression", { operator: "not", value: $2 }, @$); }
  | '-' expression
      %prec UMINUS { $$ = node("UnaryExpression", { operator: "-", value: $2 }, @$); }
  | expression[left] '+' expression[right]
      { $$ = binary("+", $left, $right, @$); }
  | expression[left] '-' expression[right]
      { $$ = binary("-", $left, $right, @$); }
  | expression[left] '*' expression[right]
      { $$ = binary("*", $left, $right, @$); }
  | expression[left] '/' expression[right]
      { $$ = binary("/", $left, $right, @$); }
  | expression[left] EQ expression[right]
      { $$ = binary("==", $left, $right, @$); }
  | expression[left] NE expression[right]
      { $$ = binary("!=", $left, $right, @$); }
  | expression[left] AND expression[right]
      { $$ = binary("and", $left, $right, @$); }
  | expression[left] OR expression[right]
      { $$ = binary("or", $left, $right, @$); }
  ;

literal
  : NUMBER
      { $$ = node("NumberLiteral", { value: Number(yytext) }, @1); }
  | STRING
      { $$ = node("StringLiteral", { value: JSON.parse(yytext) }, @1); }
  | BOOLEAN
      { $$ = node("BooleanLiteral", { value: yytext === "true" }, @1); }
  | NULL
      { $$ = node("NullLiteral", { value: null }, @1); }
  | BACKTICK TEXT BACKTICK
      { $$ = node("TemplateLiteral", { value: $2 }, @$); }
  | NUMBER IDENTIFIER
      %prec '*' { $$ = node("MeasuredLiteral", { value: $1, unit: $2 }, @$); }
  ;

semantic_probe
  : IDENTIFIER[value]
      {
        $$ = $value;
        @$ = @value;
        yysp = yysp + 0;
        yyvstack[yysp] = $$;
        yylstack[yysp] = @$;
        yyleng = yytext.length;
        yylineno = yylineno;
        yyloc = yylloc;
        yyrulelength = 1;
        yyss = yysstack;
        ##$ = ##1;
        #$ = #value;
        yy.events.push({ stack: ##value, id: #value#, negative: $-1, place: @-1 });
      }
  ;

%%

%include "shared/runtime.js"
const example = "select temperature, 'café' where station == '東京-7' 🚀";
export function parseObservatoryQuery(source, context = {}) {
  if (typeof source !== "string") throw new TypeError("source must be text");
  return parser.parse(source, context);
}
