<?hh // strict
namespace Fixture\Stress;

use namespace HH\Lib\{C, Dict, Keyset, Math, Str, Vec};
use type DateTimeImmutable;
use function sprintf;

new module fixture.stress {
}
module fixture.stress;

/**
 * Broad Hack grammar fixture.
 * @package Fixture
 * @author Mark
 */
<<__Sealed(Result::class), FixtureAttribute('東京', '𝟙')>>
interface Renderable<+T> {
  public function render(T $value): string;
}

trait Timestamped {
  private ?DateTimeImmutable $createdAt = null;

  final public function createdAt(): DateTimeImmutable {
    return $this->createdAt ?? new DateTimeImmutable('now');
  }
}

enum Status: string as string {
  READY = 'ready';
  RUNNING = 'running';
  DONE = 'done';
}

enum class Signal: string {
  string START = 'start';
  string STOP = 'stop';
}

newtype UserId as int = int;
type Row = shape(
  'id' => UserId,
  'name' => string,
  ?'tags' => vec<string>,
  ...
);

<<__ConsistentConstruct>>
abstract class Repository<Tk as arraykey, Tv as Row>
  implements Renderable<Tv> {
  use Timestamped;

  protected dict<Tk, Tv> $rows = dict[];
  const string KIND = 'fixture';

  public function __construct(private string $label = "rows") {}

  abstract protected function keyOf(Tv $value): Tk;

  final public function add(Tv $value): this {
    $this->rows[$this->keyOf($value)] = $value;
    return $this;
  }

  public function render(Tv $value): string {
    $tags = $value['tags'] ?? vec[];
    return sprintf('%s:%s[%s]', $this->label, $value['name'], Str\join($tags, '|'));
  }

  public function all(): vec<Tv> {
    return Vec\values($this->rows);
  }
}

final class UserRepository extends Repository<UserId, Row> {
  <<__Override>>
  protected function keyOf(Row $value): UserId {
    return $value['id'];
  }

  public async function fetch(UserId $id): Awaitable<?Row> {
    await RescheduleWaitHandle::create(RescheduleWaitHandle::QUEUE_DEFAULT, 0);
    return $this->rows[$id] ?? null;
  }
}

function make_row(int $id, string $name, vec<string> $tags = vec[]): Row {
  return shape('id' => $id, 'name' => $name, 'tags' => $tags);
}

function collection_examples(): void {
  $legacy = array('one' => 1, 'two' => 2);
  $empty = array();
  $vector = Vector {1, 2, 3};
  $map = Map {'alpha' => 1, 'beta' => 2};
  $pair = Pair {'left', 'right'};
  $vec = vec[0, 0x2a, 3.5e+2];
  $dict = dict['truth' => true, 'nothing' => null];
  $keys = keyset['BMP-雪', 'astral-🚀', 'G-clef-𝄞'];
  list($first, $second) = tuple($vector[0], $map['beta']);
  echo (string)($first + $second), (bool)$empty, $pair[0], C\count($keys);
}

function string_examples(string $name): vec<string> {
  $double = "hello $name; tab=\t; hex=\x41; octal=\101";
  $braced = "user={$name} and ${name}";
  $single = 'single \' quote and \\ slash';
  $sqlDouble = "SELECT id, name FROM users WHERE name = '$name'";
  $sqlSingle = 'UPDATE users SET active = 1 WHERE id = 7';
  $regexDouble = re"/^(?<word>[A-Z]+)-{$name}{1,3}$/iu";
  $regexSingle = re'/[a-z_]+\d{2,4}$/';
  return vec[$double, $braced, $single, $sqlDouble, $sqlSingle, $regexDouble, $regexSingle];
}

function document_examples(string $name): (string, string) {
  $heredoc = <<<MESSAGE
Hello {$name},
Unicode BMP 東京 λ and astral 𝌆 🛰️.
MESSAGE;
  $nowdoc = <<<'LITERAL'
$name remains literal here.
Backslashes \\ and quotes ' " remain too.
LITERAL;
  return tuple($heredoc, $nowdoc);
}

function xhp_example(Row $row): :div {
  $title = 'profile-'.$row['name'];
  return <div class="card" data-title={$title}>
    <!-- valid XHP comment -->
    <h1>{$row['name']} &amp; 東京</h1>
    <span data-kind='astral-🚀'>{Str\join($row['tags'] ?? vec[], ', ')}</span>
  </div>;
}

async function run_pipeline(UserRepository $repo, vec<Row> $input): Awaitable<dict<UserId, string>> {
  $output = dict[];
  foreach ($input as $index => $row) {
    if ($index % 2 === 0 && $row['name'] !== '') {
      $repo->add($row);
    } else if ($row['name'] is string) {
      continue;
    } else {
      break;
    }
  }

  concurrent {
    $left = await $repo->fetch(1);
    $right = await $repo->fetch(2);
  }

  for ($i = 0; $i < 3; $i++) {
    $candidate = await $repo->fetch($i);
    $output[$i] = $candidate is nonnull ? $repo->render($candidate) : 'missing';
  }

  $counter = 2;
  while ($counter-- > 0) { $output[$counter + 10] = (string)$counter; }
  do { $counter++; } while ($counter < 1);
  return $output;
}

function control_flow(mixed $value): string {
  try {
    switch ($value) {
      case Status::READY:
        return 'ready';
      case Status::RUNNING:
        throw new \RuntimeException('still running');
      default:
        return nameof Status;
    }
  } catch (\RuntimeException $error) {
    return $error->getMessage();
  } finally {
    $GLOBALS['fixture_seen'] = true;
  }
}

function operator_examples(int $left, int $right): int {
  $sum = $left + $right * 2 - 1;
  $bits = ($sum << 1) | ($right & 0xff);
  $same = $left === $right || $left != 0;
  $next = $same ? $bits : ~$bits;
  $next += 3;
  return vec[$next] |> $$[0];
}

<<__EntryPoint>>
async function main(): Awaitable<void> {
  require_once 'vendor/autoload.php';
  $repo = new UserRepository('users');
  $rows = vec[
    make_row(1, 'Ada', vec['math', 'λ']),
    make_row(2, 'Grace', vec['compiler', '東京']),
  ];
  $result = await run_pipeline($repo, $rows);
  $callback = function(int $id): string use ($result) { return $result[$id] ?? 'none'; };
  $invoke = $callback(1);
  echo $invoke, control_flow(Status::READY), operator_examples(3, 4);
  echo xhp_example($rows[0]);
}
