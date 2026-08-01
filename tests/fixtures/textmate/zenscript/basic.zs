import crafttweaker.api.item.IItemStack;

// Basic ZenScript fixture: café 東京 λ 🚀 𝌆
val count as int = 42;
var ratio as double = 6.02E+23D;
val mask as uint = 0xCAFEu;
val recipe = <minecraft:stone:*>;
val greeting as string = "café 東京 λ 🚀 𝌆\nready";
val label as string = 'single\tquoted';
val stacks as crafttweaker.api.item.IItemStack[] = [recipe];

/* A closed block comment with Unicode: café 東京 λ 🚀 𝌆. */
public zenClass BasicFixture {
    private val name as string;

    zenConstructor(name as string) {
        this.name = name;
    }

    public function describe(extra as string) as string {
        if (extra != null && extra.length > 0) {
            return this.name + ": " + extra;
        } else {
            return "empty";
        }
    }
}
