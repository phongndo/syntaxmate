// Cairo 0 grammar stress fixture.
// Unicode stays in comments and strings: café λ 東京 🚀 𝌆
from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.math import assert_nn, assert_not_zero
from starkware.cairo.common.registers import get_ap, get_fp_and_pc

const ZERO = 0;
const ONE = +1;
const NEGATIVE = -7;
const MASK = 0xCAFE;
const MESSAGE = 'café λ 東京 🚀 𝌆';

%{
# Embedded Python remains open across several tokenizer lines.
labels = ["café", "λ", "東京", "🚀", "𝌆"]
encoded = "|".join(labels).encode("utf-8")
ids.python_word = len(encoded)
%}

struct Point {
    x: felt,
    y: felt,
}

struct Span {
    start: felt*,
    end: felt*,
}

struct Context {
    origin: Point,
    count: felt,
    data: felt*,
}

namespace Geometry {
    const DIMENSIONS = 2;

    func origin() -> (point: Point) {
        let point = Point(x=0, y=0);
        return (point=point);
    }

    func translate(point: Point, dx: felt, dy: felt) -> (result: Point) {
        alloc_locals;
        local result: Point;
        assert result.x = point.x + dx;
        assert result.y = point.y + dy;
        return (result=result);
    }

    namespace Metrics {
        func taxicab(point: Point) -> (distance: felt) {
            let ax = abs(point.x);
            let ay = abs(point.y);
            return (distance=ax + ay);
        }
    }
}

func read_registers() -> (ap_value: felt, fp_value: felt) {
    let (ap_value) = get_ap();
    let (fp_value, pc_value) = get_fp_and_pc();
    tempvar relative = pc_value - fp_value;
    return (ap_value=ap_value, fp_value=fp_value);
}

func memory_demo{range_check_ptr}(values: felt*, length: felt) -> (sum: felt) {
    alloc_locals;
    local scratch: felt*;
    let (scratch) = alloc();
    assert [scratch] = length;
    assert [scratch + 1] = values[0];
    assert [ap] = [fp - 3], ap++;
    tempvar copied = [ap - 1];
    let typed = cast(scratch, felt*);
    let newer = new Context(
        origin=Point(x=1, y=2),
        count=length,
        data=typed,
    );
    assert newer.count = length;
    return (sum=copied + newer.count);
}

func branch_demo(value: felt) -> (result: felt) {
    alloc_locals;
    local result;
    if (value == 0) {
        assert result = ZERO;
    } else {
        if (value == ONE) {
            assert result = 10;
        } else {
            assert result = value + MASK;
        }
    }
    return (result=result);
}

func guarded_division{range_check_ptr}(numerator: felt, denominator: felt) -> (q: felt) {
    with_attr error_message("denominator must be nonzero — café 🚀") {
        assert_not_zero(denominator);
    }
    let q = numerator / denominator;
    with range_check_ptr {
        assert_nn(q);
    }
    return (q=q);
}

func hint_demo(seed: felt) -> (chosen: felt) {
    alloc_locals;
    local chosen;
    %{ 
# A multiline hint exercises embedded Python state inside a Cairo function.
candidate = ids.seed if ids.seed >= 0 else 0
ids.chosen = candidate + len("東京🚀")
    %}
    let nondeterministic = nondet %{ memory[ap] = ids.seed + 1 %};
    assert chosen = nondeterministic;
    return (chosen=chosen);
}

func loop_demo(limit: felt) -> (total: felt) {
    alloc_locals;
    local total;
    assert total = 0;
    tempvar index = 0;

    loop_start:
    if (index == limit) {
        jmp rel loop_done;
    }
    assert total = total + index;
    tempvar index = index + 1;
    jmp rel loop_start;

    loop_done:
    return (total=total);
}

func opcode_demo(flag: felt) -> (value: felt) {
    alloc_locals;
    local value;
    call rel helper;
    jmp rel selected if flag != 0;
    assert value = [fp - 3];
    jmp rel finished;

    selected:
    assert value = [ap - 1];

    finished:
    return (value=value);

    helper:
    [ap] = 42, ap++;
    ret;
}

func metadata_demo() -> (size: felt) {
    alloc_locals;
    local item;
    assert item = SIZEOF_LOCALS;
    static_assert SIZEOF_LOCALS >= 1;
    let offset = codeoffset(metadata_label);
    let size = cast(offset, felt);
    return (size=size + item);

    metadata_label:
    dw 0x1234;
}

namespace Storage {
    func write_pair(pointer: felt*, left: felt, right: felt) {
        assert [pointer] = left;
        assert [pointer + 1] = right;
        return ();
    }

    func read_pair(pointer: felt*) -> (left: felt, right: felt) {
        let left = [pointer];
        let right = [pointer + 1];
        return (left=left, right=right);
    }
}

func main{range_check_ptr}() {
    alloc_locals;
    let (base) = alloc();
    Storage.write_pair(base, 13, 21);
    let (left, right) = Storage.read_pair(base);
    let (point) = Geometry.translate(
        Point(x=left, y=right),
        NEGATIVE,
        +3,
    );
    let (distance) = Geometry.Metrics.taxicab(point);
    let (safe) = guarded_division(distance, ONE);
    let summary = "café λ 東京 🚀 𝌆";
    with_attr documentation(summary) {
        assert safe = safe;
    }
    return ();
}
