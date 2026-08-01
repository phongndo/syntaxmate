declare(strict_types=1);

/**
 * Small grammar fixture with café, 東京, λ, 🚀, and astral 𝌆.
 */
#[Attribute]
final class Greeting {
    public function __construct(
        public readonly string $name,
        private array $tags = ['basic', 'unicode'],
    ) {}

    public function render(int $count = 1): string {
        $label = match ($count) {
            0 => 'none',
            1 => "café {$this->name} 🚀 𝌆",
            default => "{$count} visits to 東京",
        };
        return $label . ': ' . implode(',', $this->tags);
    }
}

$greeting = new Greeting(name: 'Mark');
echo $greeting->render(count: 2);
