<?hh // strict
namespace Fixture\Basic;

use type HH\Lib\{C, Str};

/** Small generic value object for 東京. */
<<__ConsistentConstruct>>
final class Greeting<T as arraykey> {
  public function __construct(private T $id, private string $name) {}

  public function render(bool $excited = true): string {
    $mark = $excited ? "!" : ".";
    return "Hello, {$this->name}{$mark} λ 🚀";
  }
}

function summarize(vec<string> $names): shape('count' => int, 'text' => string) {
  $clean = Vec\map($names, $name ==> Str\trim($name));
  $labels = dict['東京' => 1, 'astral-𝌆' => 2];
  return shape('count' => C\count($clean), 'text' => Str\join($clean, ", "));
}

<<__EntryPoint>>
function main(): void { echo (new Greeting<int>(7, 'Hack'))->render(); }
