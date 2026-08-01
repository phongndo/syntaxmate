import crafttweaker.api.item.IItemStack;
import crafttweaker.api.recipe.IRecipeManager;
import mods.example.widgets.Widget;

// Stress fixture for grammar alternatives: café 東京 λ 🚀 𝌆
/* Every construct in this file is closed.
 * Strings and comments intentionally contain BMP and astral Unicode: café 東京 λ 🚀 𝌆.
 */

val zero = 0;
val integer = 123456;
val trailingPoint = 12.;
val decimal = 12.375;
val leadingPoint = .625;
val positiveExponent = 6.022E+23D;
val negativeExponent = 1.0e-9f;
val floatSuffix = 7F;
val doubleSuffix = 8d;
val longSuffix = 99L;
val unsignedSuffix = 100u;
val unsignedLongSuffix = 101UL;
val lowerUnsignedLong = 102ul;
val hexLower = 0xdeadbeef;
val hexUpper = 0XCAFE;
val binaryLower = 0b101010;
val binaryUpper = 0B1100;
val octalLower = 0o755;
val octalUpper = 0O644;
val basedWithSeparators = 0b1010_0110;
val negativeBased = -0x2A;

val doubleQuoted = "double: café 東京 λ 🚀 𝌆\n\t\\\"done";
val singleQuoted = 'single: café 東京 λ 🚀 𝌆\n\t\\\'done';
val emptyDouble = "";
val emptySingle = '';

val stone = <minecraft:stone>;
val polished = <minecraft:polished_andesite:2>;
val wildcard = <minecraft:wool:*>;
val oreEntry = <ore:ingotIron>;
val water = <liquid:water>;
val craftingType = <recipetype:crafting>;

val primitiveFlags as bool[] = [true, false, true];
val stackArray as IItemStack[] = [stone, polished];
val qualifiedArray as crafttweaker.api.item.IItemStack[] = [wildcard];
val widgetArray as mods.example.widgets.Widget[] = [];
var nestedObjects as Widget[][] = [widgetArray];

alias Stack = IItemStack;

@Native("mods.example.NativePoint")
public struct Point {
    val x as double;
    val y as double;
}

export enum Direction {
    NORTH,
    EAST,
    SOUTH,
    WEST
}

public interface Named {
    function getName() as string;
}

abstract class AbstractTask implements Named {
    protected immutable val id as long;

    public zenConstructor(id as long) {
        this.id = id;
    }

    public abstract function run() as void;
}

@Precondition("example_loaded")
export zenClass StressFixture extends AbstractTask implements Named {
    private static const LIMIT as int = 16;
    internal var enabled as bool = true;
    protected val manager as IRecipeManager;
    public val title as string;

    public zenConstructor(manager as IRecipeManager, title as string) {
        super(1L);
        this.manager = manager;
        this.title = title;
    }

    public override function getName() as string {
        return this.title;
    }

    public final override function run() as void {
        var total as int = 0;
        for index in 0 .. LIMIT {
            if (index % 2 == 0) {
                total += index;
                continue;
            } else if (index >= 11) {
                break;
            } else {
                total++;
            }
        }

        do {
            total--;
        } while (total > 8);

        while (enabled && total <= LIMIT) {
            total *= 2;
            enabled = false;
        }

        switch total {
            case 0:
                this.log("zero");
                break;
            case 16:
                this.log("limit");
                break;
            default:
                this.log("other");
                break;
        }
    }

    private function log(message as string) as void {
        print(this.title + ".log: " + message);
    }

    public function classify(value as any) as string {
        if (value instanceof Widget) {
            val widget = value as Widget;
            return widget.name;
        }
        if (value is string) {
            return value as string;
        }
        return match value {
            null => "null",
            true => "true",
            false => "false",
            default => "other"
        };
    }

    public function guarded(action as function() as void) as void throws Exception {
        try {
            lock this.manager {
                action();
            }
        } catch error as Exception {
            throw error;
        } finally {
            this.log("finished");
        }
    }

    public function nullable(widget as Widget?) as string {
        return widget?.name ?? "missing";
    }

    public function arithmetic(left as int, right as int) as int {
        var result = (left + right) * 2;
        result -= left / 2;
        result %= 7;
        result |= 1;
        result &= 15;
        result ^= 3;
        result <<= 1;
        result >>= 1;
        return result;
    }
}

expand Widget {
    public function decoratedName(prefix as string) as string {
        return prefix + this.name;
    }
}

variant Result {
    Success(string),
    Failure(string)
}

extern function nativeLookup(name as string) as Widget;

function makeWidget(name as string) as Widget {
    val created = new Widget(name);
    return created;
}

function choose(flag as bool) as string {
    return flag ? "yes" : "no";
}

function failFast(message as string) as void {
    if (message.length == 0) {
        panic "empty message";
    }
}

val dottedName = crafttweaker.api.item.IItemStack;
val rangeClosed = 1 .. 5;
val rangeOpen = 1 ... 5;
val lambda = (value as int) => value * value;
val equality = (1 === 1) && (2 !== 3);
val shifts = (8 >>> 1) + (1 << 3);
