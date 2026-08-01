// Cairo 0 basics: café λ 東京 🚀 𝌆
from starkware.cairo.common.math import assert_nn

const LIMIT = 0x10;

struct Point {
    x: felt,
    y: felt,
}

func add_points(a: Point, b: Point) -> (result: Point) {
    alloc_locals;
    local result: Point;
    assert result.x = a.x + b.x;
    assert result.y = a.y + b.y;
    if (result.x != 0) {
        let note = 'café 東京 🚀 𝌆';
        assert_nn(result.x);
    } else {
        tempvar fallback = -1;
    }
    return (result=result);
}
