<?php
declare(strict_types=1);

/** Modern PHP grammar stress fixture: café, λ, 漢字, 😀. */
namespace Mark\Fixtures\Stress;

use ArrayIterator;
use Attribute;
use Countable;
use DateTimeImmutable;
use Iterator;
use JsonException;
use RuntimeException;
use Stringable;
use function array_map;
use const JSON_THROW_ON_ERROR;

const APP_NAME = 'Syntax Café', MAX_RESULTS = 25;
#[Attribute(Attribute::TARGET_ALL | Attribute::IS_REPEATABLE)]
final class Example {
    public function __construct(public string $name, public array $options = []) {}
}
#[Example('contract')]
interface Renderable extends Stringable {
    public function render(?callable $formatter = null): string;
}
interface Source {
    public function records(): (Iterator&Countable)|null;
}

trait HasMetadata {
    /** @var array<string, mixed> */
    private array $metadata = [];
    // Coalescing keeps absent keys distinct from stored values.
    public function metadata(string $key, mixed $default = null): mixed {
        return $this->metadata[$key] ?? $default;
    }
    public function withMetadata(string $key, mixed $value): static {
        $this->metadata[$key] = $value;
        return $this;
    }
}

enum Status: string {
    case Draft = 'draft';
    case Published = 'published';
    case Archived = 'archived';
    public function label(): string {
        return match ($this) {
            self::Draft => 'Draft ✎',
            self::Published => 'Published ✓',
            self::Archived => 'Archived ⌁',
        };
    }
}
enum Priority {
    case Low;
    case Normal;
    case High;
}

#[Example('model', ['format' => 'html'])]
final class Report implements Renderable, Source {
    use HasMetadata;
    public const VERSION = 2;
    private static int $instances = 0;
    public function __construct(
        public readonly string $id,
        public string $title,
        private Status $status = Status::Draft,
        protected ?DateTimeImmutable $createdAt = null,
        public array $tags = [],
    ) {
        self::$instances++;
    }
    public static function count(): int { return self::$instances; }
    public function records(): (Iterator&Countable)|null {
        return new ArrayIterator($this->tags);
    }
    public function render(?callable $formatter = null): string {
        $label = $this->status->label();
        $text = "{$this->title} — {$label}";
        return $formatter !== null ? $formatter($text) : $text;
    }
    public function timestamp(): string {
        return $this->createdAt?->format(DateTimeImmutable::ATOM) ?? 'never';
    }
    public function __toString(): string {
        return $this->render();
    }
}

/** @return list<int> */
function sequence(int $start, int $end): iterable {
    for ($number = $start; $number <= $end; $number++) {
        yield $number => $number * $number;
    }
    yield from [999, 1000];
}

function buildReport(string $title, Status $status = Status::Draft, string ...$tags): Report {
    return new Report(
        id: bin2hex(random_bytes(4)),
        title: $title,
        status: $status,
        createdAt: new DateTimeImmutable('2024-01-02T03:04:05+00:00'),
        tags: $tags,
    );
}

$report = buildReport(
    title: 'Grammar 漢字 😀',
    status: Status::Published,
    tags: 'php',
);
$report->withMetadata('priority', Priority::High);
$counter = 0;
$upper = static function (string $value) use (&$counter): string {
    $counter++;
    return $value;
};
$trimmed = fn(string $value): string => trim($value);
$render = $report->render(...);
$values = [1, 2, 3, ...[4, 5]];
[$first, $second] = $values;
$rest = array_slice($values, 2);
['host' => $host, 'port' => $port] = ['host' => 'localhost', 'port' => 5432];
$mapped = array_map(fn(int $n): int => $n ** 2, $values);
$sqlStatus = $report->metadata('status', 'published');
$sql = <<<SQL
SELECT id, title
FROM reports
WHERE status = '{$sqlStatus}'
  AND title LIKE '%café%'
ORDER BY created_at DESC;
SQL;

$json = <<<'JSON'
{
  "language": "PHP",
  "unicode": "café λ 漢字 😀",
  "enabled": true,
  "items": [1, 2, 3]
}
JSON;

try {
    $decoded = json_decode($json, associative: true, flags: JSON_THROW_ON_ERROR);
} catch (JsonException|RuntimeException $error) {
    $decoded = ['error' => $error->getMessage()];
} finally {
    $counter += 1;
}
foreach (sequence(1, 3) as $key => $square) {
    if ($key === 2) {
        continue;
    }
    $counter += $square;
}
while ($counter < 5) {
    $counter++;
}
do {
    $counter--;
} while ($counter > 100);
switch ($report->metadata('priority')) {
    case Priority::High:
        $badge = 'urgent';
        break;
    default:
        $badge = 'normal';
}
?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title><?= $report->title ?></title>
  <style>
    :root { --accent: #7c3aed; color-scheme: light dark; }
    body { font: 16px/1.5 system-ui, sans-serif; margin: 2rem; }
    .report[data-priority="urgent"] { border-inline-start: 4px solid var(--accent); }
    code::before { content: "λ "; }
  </style>
  <script type="application/json" id="config">
    {"locale":"fr-FR","greeting":"café 漢字 😀","features":["match","enum"]}
  </script>
  <script>
    const config = JSON.parse(document.querySelector('#config').textContent);
    const message = `${config.greeting}: ${config.features.join(', ')}`;
    window.addEventListener('DOMContentLoaded', () => console.info(message));
  </script>
</head>
<body>
  <!-- HTML embedding with PHP islands; café λ 漢字 😀 -->
  <main class="report" data-priority="<?= $badge ?>">
    <h1><?= $render($upper) ?></h1>
    <p>Created: <?= $report->timestamp() ?></p>
    <ul>
      <?php foreach ($mapped as $index => $value): ?>
        <li data-index="<?= $index ?>"><?= $value ?></li>
      <?php endforeach; ?>
    </ul>
    <pre><?= $sql ?></pre>
  </main>
</body>
</html>
